# Experiment 14 (proof of concept, decision D15): reads a ripped title's bitmap subtitle tracks with Tesseract so
# that a later script can tell SDH, regular, forced and commentary subtitle tracks apart by their text. Input: the
# subtitle-only evidence.mks that probe-title.ps1 left in $Staging\<Slug>\<Label>\. For every subtitle stream of
# that file (or those given with -Streams): ffmpeg draws the stream on a black canvas, keeps one frame per change
# (fps=<Fps>, mpdecimate), inverts it (dark text on white, which Tesseract reads best) and writes PNG frames named
# by their time to a work folder on C:; tesseract then reads all of a stream's frames in one call.
#
# In $Recordings\<Slug>\<Label>\subs-ocr\ (small text files, pulled into the corpus; no subtitle text beyond the
# examples in summary.json):
#   streams.json         ffprobe's view of evidence.mks
#   ffmpeg-<n>.txt       ffmpeg's messages for stream s:<n> (.stderr holds the showinfo line of every kept frame)
#   sub-<n>.frames.json  one record per kept frame: frame number, file, time in seconds, characters, lines and the
#                        marker counts of its text
#   summary.json         per stream: frames, frames with text, text lines, marker counts (bracketed lines, music
#                        notes, lines beginning with #, speaker labels, upper-case lines), run times, and up to
#                        <Examples> example lines per marker, each cut at 60 characters. Written last.
# In $TuningData\<Slug>\<Label>\subs-ocr\ (the tuning dataset, decision D16; the full text stays there):
#   s<n>\f-<pts>.png     the frames, so that OCR can be re-run with other settings without the rip
#   s<n>\list.txt        the frame list given to tesseract
#   s<n>\tesseract.txt   tesseract's raw output (pages separated by form feeds) with .stderr/.timing.jsonl/.exit.json
#   sub-<n>.ocr.jsonl    one line per frame: frame number, file, time and the text (lines joined with " | ")
# The log shows the first <Show> frames with text of every stream so that the OCR quality can be judged.
# An existing subs-ocr folder (in either place) is refused unless -Force is given.
# The last line is "RESULT ok" or "RESULT FAILED: <reason>"; after a failure the script throws.
param(
    [Parameter(Mandatory)][string]$Slug,
    [Parameter(Mandatory)][string]$Label,
    [int[]]$Streams = @(),
    [int]$Fps = 4,
    [string]$Canvas = '',
    [string]$Lang = 'eng',
    [int]$Psm = 6,
    [int]$Show = 30,
    [int]$Examples = 12,
    [switch]$Force
)
. C:\lazymkv\src\scripts\phase0\env.ps1

$FF = [string][char]12
$NOTE = [string][char]0x266A + [char]0x266B

function Count-Markers([string[]]$Lines) {
    $m = [ordered]@{ bracket = 0; note = 0; hash = 0; speaker = 0; upper = 0 }
    $ex = @{ bracket = @(); note = @(); hash = @(); speaker = @(); upper = @() }
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

try {
    Assert-Slug $Slug
    Assert-Slug $Label
    if ($Fps -lt 1 -or $Fps -gt 25) { throw "-Fps must be between 1 and 25" }
    if ($Canvas -and $Canvas -notmatch '^\d{2,4}x\d{2,4}$') { throw "-Canvas must look like 1920x1080" }
    if (-not $FFMPEG -or -not $FFPROBE) { throw 'ffmpeg/ffprobe not found' }
    if (-not $TESSERACT) { throw 'tesseract.exe not found (winget install UB-Mannheim.TesseractOCR)' }
    if (-not (Test-Path -LiteralPath $CAP)) { throw "missing $CAP; build it with: scripts/box.sh cargo build --release -p lazymkv-capture" }
    if (-not $TuningData) { throw "`$TuningData is not set (C:\lazymkv\phase0\config.ps1)" }

    $evidence = "$Staging\$Slug\$Label\evidence.mks"
    if (-not (Test-Path -LiteralPath $evidence)) { throw "no $evidence; run probe-title.ps1 first" }
    $rec = "$Recordings\$Slug\$Label"
    if (-not (Test-Path -LiteralPath $rec)) { throw "no recording $rec; run probe-title.ps1 first" }
    $out = "$rec\subs-ocr"
    $tune = "$TuningData\$Slug\$Label\subs-ocr"
    $work = "$P0\work\subs-ocr\$Slug-$Label"
    foreach ($folder in @($out, $tune)) {
        if ((Test-Path -LiteralPath $folder) -and -not $Force) { throw "$folder already exists and is kept as evidence; use -Force to replace it" }
    }
    foreach ($folder in @($out, $tune, $work)) {
        if (Test-Path -LiteralPath $folder) { Remove-Item -LiteralPath $folder -Recurse -Force -ErrorAction Stop }
        New-Item -ItemType Directory -Force -Path $folder -ErrorAction Stop | Out-Null
    }
    "ocr: $evidence tesseract=$TESSERACT lang=$Lang psm=$Psm fps=$Fps"

    & $CAP record --out "$out\streams.json" -- $FFPROBE -v error -show_streams -show_format -of json $evidence | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "ffprobe exited $LASTEXITCODE; see streams.json.stderr" }
    $probe = Get-Content -LiteralPath "$out\streams.json" -Raw -ErrorAction Stop | ConvertFrom-Json
    $subs = @($probe.streams | Where-Object { $_.codec_type -eq 'subtitle' })
    if ($subs.Count -eq 0) { throw "$evidence has no subtitle stream" }
    $duration = [double]$probe.format.duration
    if ($duration -le 0) { throw "ffprobe reports no duration for $evidence" }
    $wanted = if ($Streams.Count -gt 0) { $Streams } else { 0..($subs.Count - 1) }
    foreach ($n in $wanted) { if ($n -lt 0 -or $n -ge $subs.Count) { throw "no subtitle stream s:$n (the file has $($subs.Count))" } }

    $summary = [ordered]@{
        slug = $Slug; label = $Label; evidence = $evidence; duration_s = [math]::Round($duration, 3)
        fps = $Fps; lang = $Lang; psm = $Psm; tesseract = $TESSERACT; run = (Get-Date).ToString('s'); streams = @()
    }
    foreach ($n in $wanted) {
        $s = $subs[$n]
        $size = if ($Canvas) { $Canvas } elseif ([int]$s.width -gt 0 -and [int]$s.height -gt 0) { "$($s.width)x$($s.height)" } else { '1920x1080' }
        $sdir = "$work\s$n"
        New-Item -ItemType Directory -Force -Path $sdir -ErrorAction Stop | Out-Null
        # sub2video: ffmpeg turns the bitmap subtitle stream into video frames of -canvas_size, overlaid on a black
        # canvas that ticks at -Fps; mpdecimate keeps only the frames that differ from the one before.
        $graph = "color=c=black:s=${size}:r=${Fps}:d=${duration}[bg];[bg][0:s:${n}]overlay=format=auto[ov];[ov]mpdecimate,negate,format=gray,showinfo[v]"
        $t0 = Get-Date
        & $CAP record --out "$out\ffmpeg-$n.txt" -- $FFMPEG -hide_banner -nostats -v info -y -canvas_size $size -i $evidence `
            -filter_complex $graph -map '[v]' -fps_mode vfr -frame_pts 1 -f image2 "$sdir\f-%08d.png" | Out-Null
        $ffmpegS = [math]::Round(((Get-Date) - $t0).TotalSeconds, 1)
        if ($LASTEXITCODE -ne 0) { throw "ffmpeg exited $LASTEXITCODE on stream s:${n}; see ffmpeg-$n.txt.stderr" }
        $frames = @(Get-ChildItem -LiteralPath $sdir -Filter 'f-*.png' -ErrorAction Stop | Sort-Object Name)
        if ($frames.Count -eq 0) { throw "ffmpeg wrote no frame for stream s:${n}; see ffmpeg-$n.txt.stderr" }
        # showinfo prints one line per kept frame, in output order: its pts_time is the frame's time.
        $info = @(Select-String -LiteralPath "$out\ffmpeg-$n.txt.stderr" -Pattern 'Parsed_showinfo.*\bn:\s*(\d+).*\bpts_time:\s*([0-9.]+)' -ErrorAction Stop)
        $times = @{}
        foreach ($line in $info) { $times[[int]$line.Matches[0].Groups[1].Value] = [double]$line.Matches[0].Groups[2].Value }
        if ($times.Count -ne $frames.Count) { "warning: s:${n} showinfo lines=$($times.Count) frames=$($frames.Count); times come from the file names" }

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
            $mk = Count-Markers $lines
            $records += [ordered]@{ frame = $k; file = $frames[$k].Name; t = $t; chars = ($text -replace ' \| ', '').Length; lines = $lines.Count; markers = $mk.counts }
            $jsonl += (ConvertTo-Json -Compress ([ordered]@{ frame = $k; file = $frames[$k].Name; t = $t; text = $text }))
            $allLines += $lines
            if ($lines.Count -gt 0 -and $shown -lt $Show) { "  s:${n} t=$t | $text"; $shown++ }
        }
        $withText = @($records | Where-Object { $_.chars -gt 0 }).Count
        $all = Count-Markers $allLines
        "stream s:${n} lang=$($s.tags.language) codec=$($s.codec_name) canvas=$size frames=$($frames.Count) text_frames=$withText lines=$($allLines.Count) " +
            "bracket=$($all.counts.bracket) note=$($all.counts.note) hash=$($all.counts.hash) speaker=$($all.counts.speaker) upper=$($all.counts.upper) ffmpeg_s=$ffmpegS tesseract_s=$tessS"
        [IO.File]::WriteAllLines("$work\sub-$n.ocr.jsonl", [string[]]$jsonl, (New-Object Text.UTF8Encoding $false))
        Set-Content -LiteralPath "$out\sub-$n.frames.json" -Value (ConvertTo-Json -Depth 4 @($records)) -Encoding UTF8 -ErrorAction Stop
        $summary.streams += [ordered]@{
            n = $n; language = $s.tags.language; codec = $s.codec_name; canvas = $size
            frames = $frames.Count; text_frames = $withText; lines = $allLines.Count
            markers = $all.counts; examples = $all.examples; ffmpeg_s = $ffmpegS; tesseract_s = $tessS
        }
        # The frames, tesseract's output and the full text go to the tuning dataset; nothing bulky stays on C:.
        Copy-Item -LiteralPath $sdir -Destination "$tune\s$n" -Recurse -ErrorAction Stop
        Copy-Item -LiteralPath "$work\sub-$n.ocr.jsonl" -Destination "$tune\sub-$n.ocr.jsonl" -ErrorAction Stop
    }
    Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction Stop
    Set-Content -LiteralPath "$out\summary.json" -Value (ConvertTo-Json -Depth 6 $summary) -Encoding UTF8 -ErrorAction Stop
    "written: $out and $tune"
    'RESULT ok'
} catch {
    "RESULT FAILED: $($_.Exception.Message)"
    throw
}
