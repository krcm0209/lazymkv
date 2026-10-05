# Experiment 13: measures a ripped title's audio streams so that audio_similarity.py can tell a commentary, an
# exact duplicate or programme audio apart by content. One ffmpeg pass over the one .mkv in
# $Staging\<Slug>\<Label>\ writes, for the main stream and every candidate stream, the speech-band (300-3400 Hz)
# RMS level of every 20 ms, and the MD5 of each of those streams' packets. Small text files only, in
# $Recordings\<Slug>\<Label>\audio-levels-20ms-<Main>-<Cand>[-<Cand>...]\ :
#   manifest.txt   first line "# lazymkv audio-levels v2 (experiment 13)"; then the file, the run time and
#                  frame_ms=20; then one line per stream: number, codec, channels, layout, language, frames,
#                  MD5 of its packets (identical MD5 = exact duplicate). audio_similarity.py refuses a folder
#                  without it, so the first measurement's folders (audio-levels-<Main>-<Cand>, 100 ms) are never read.
#   main-mix.txt   the main stream mixed down to mono: a header line "# lazymkv audio levels v2: frame_ms=20 ...",
#                  then one level in dB per line, 0.1 dB steps, -100 = silence
#   main-fc.txt    the main stream's centre channel (dialogue); only when its layout has one besides other channels
#   cand-<n>.txt   candidate stream <n> mixed down to mono
#   streams.json   ffprobe's list of the file's streams
#   ffprobe.txt    ffprobe's messages
#   streamhash.txt ffmpeg's MD5 of each stream's packets, in the order main, candidates
#   ffmpeg.txt     ffmpeg's messages
# Whether the main stream has a centre channel is read from the installed ffmpeg's own table of channel layouts
# (ffmpeg -layouts), not from a list kept here. A layout that ffmpeg does not name and ffprobe does not spell out
# (for example "unknown") counts as having none: the main stream's mixdown is then the dialogue curve.
# Stream numbers are ffprobe's stream indexes (0 is normally the video). Stereo width is not measured.
# Nothing is written to the share. An existing folder is kept unless -Force is given.
# The last line is "RESULT ok" or "RESULT FAILED: <reason>"; after a failure the script throws.
param(
    [Parameter(Mandatory)][string]$Slug,
    [Parameter(Mandatory)][string]$Label,
    [Parameter(Mandatory)][int]$Main,
    [Parameter(Mandatory)][int[]]$Cand,
    [switch]$Force
)
. C:\lazymkv\src\scripts\phase0\env.ps1

$Key = 'lavfi.astats.Overall.RMS_level='

# Turns ffmpeg's ametadata output into one value per line (0.1 dB, -100 for silence). Returns the frame count.
function Convert-Levels([string]$Raw, [string]$Out, [string]$Header) {
    $inv = [Globalization.CultureInfo]::InvariantCulture
    $writer = New-Object System.IO.StreamWriter($Out, $false, (New-Object System.Text.UTF8Encoding($false)))
    $count = 0
    try {
        $writer.WriteLine($Header)
        foreach ($line in [System.IO.File]::ReadLines($Raw)) {
            if (-not $line.StartsWith($Key)) { continue }
            $text = $line.Substring($Key.Length)
            $value = -100.0
            if (-not ($text.Contains('inf') -or $text.Contains('nan'))) {
                $value = [math]::Max(-100.0, [double]::Parse($text, $inv))
            }
            $writer.WriteLine($value.ToString('0.0', $inv))
            $count++
        }
    } finally { $writer.Close() }
    $count
}

# A manifest value without spaces.
function Get-Word($Value) {
    $text = [string]$Value
    if (-not $text) { return '-' }
    $text -replace '\s+', '_'
}

# The installed ffmpeg's own table of named channel layouts (ffmpeg -layouts, section "NAME DECOMPOSITION"):
# name -> its channels, for example 5.1(side) -> FL FR FC LFE SL SR.
function Get-LayoutTable {
    $table = @{}
    $inTable = $false
    foreach ($line in @(& $FFMPEG -hide_banner -layouts)) {
        $cols = @(([string]$line).Trim() -split '\s+')
        if ($cols.Count -ne 2) { continue }
        if ($cols[0] -eq 'NAME') { $inTable = ($cols[1] -eq 'DECOMPOSITION'); continue }
        if ($inTable) { $table[$cols[0]] = @($cols[1] -split '\+') }
    }
    if ($table.Count -eq 0) { throw "ffmpeg -layouts printed no table of channel layouts (no NAME DECOMPOSITION section): $FFMPEG" }
    $table
}

# The channels of a layout as ffprobe names it: a named layout from ffmpeg's table; an unnamed one from ffprobe's
# own spelling "<n> channels (FL+FR+...)". Anything else ("unknown", "<n> channels", empty) has no known channels.
function Get-LayoutChannels([string]$Layout, $Table) {
    if ($Layout -and $Table.ContainsKey($Layout)) { return @($Table[$Layout]) }
    if ($Layout -match '^\d+ channels \(([^()]+)\)$') { return @($Matches[1] -split '\+' | ForEach-Object { ($_ -split '@')[0] }) }
    @()
}

try {
    Assert-Slug $Slug
    Assert-Slug $Label
    if (-not $FFMPEG) { throw 'ffmpeg not found' }
    if (-not $FFPROBE) { throw 'ffprobe not found' }
    $Cand = @($Cand)
    if ($Cand -contains $Main) { throw "stream $Main is the main stream and cannot also be a candidate" }
    if (@($Cand | Select-Object -Unique).Count -ne $Cand.Count) { throw "a candidate stream is listed twice: $($Cand -join ',')" }
    $all = @($Main) + $Cand
    $name = 'audio-levels-20ms-' + ($all -join '-')
    Assert-Slug $name
    $layoutTable = Get-LayoutTable
    if (-not (Test-Path -LiteralPath $Staging)) { throw "cannot reach the staging share $Staging" }
    $dir = "$Staging\$Slug\$Label"
    if (-not (Test-Path -LiteralPath $dir)) { throw "expected one .mkv in $dir, found 0 (the folder does not exist: the rip is not in staging)" }
    $mkv = @(Get-ChildItem -LiteralPath $dir -File -ErrorAction Stop | Where-Object { $_.Extension -eq '.mkv' })
    if ($mkv.Count -ne 1) { throw "expected one .mkv in $dir, found $($mkv.Count)" }
    $file = $mkv[0].FullName
    $out = "$Recordings\$Slug\$Label\$name"
    if ((Test-Path -LiteralPath $out) -and -not $Force) { throw "$out already exists and is kept as evidence; use -Force only where the plan says so" }
    if (Test-Path -LiteralPath $out) { Remove-Item -LiteralPath $out -Recurse -Force -ErrorAction Stop }
    New-Item -ItemType Directory -Force -Path $out -ErrorAction Stop | Out-Null

    # Which streams the file has, and whether the main stream has a centre channel.
    # File names are relative (the tools run inside $out): a drive colon cannot be written in a filter argument.
    Push-Location -LiteralPath $out
    try {
        $probeLog = & cmd /c "`"$FFPROBE`" -v error -show_entries stream=index,codec_type,codec_name,profile,channels,channel_layout:stream_tags=language -of json -o streams.json `"$file`" 2>&1"
        $code = $LASTEXITCODE
    } finally { Pop-Location }
    Set-Content -LiteralPath "$out\ffprobe.txt" -Value $probeLog -Encoding UTF8 -ErrorAction Stop
    if ($code -ne 0 -or -not (Test-Path -LiteralPath "$out\streams.json")) { throw "ffprobe failed (exit $code): $(@($probeLog) | Select-Object -First 3)" }
    $streams = @((Get-Content -LiteralPath "$out\streams.json" -Raw -Encoding UTF8 -ErrorAction Stop | ConvertFrom-Json).streams)
    $info = @{}
    foreach ($n in $all) {
        $s = @($streams | Where-Object { $_.index -eq $n }) | Select-Object -First 1
        if (-not $s) { throw "the file has no stream $n" }
        if ($s.codec_type -ne 'audio') { throw "stream $n is $($s.codec_type), not audio" }
        $info[$n] = $s
    }
    # A centre channel besides other channels: for mono the centre is the mixdown itself.
    $layout = [string]$info[$Main].channel_layout
    $channels = @(Get-LayoutChannels $layout $layoutTable)
    $hasFc = ($channels -contains 'FC') -and ($channels.Count -gt 1)

    # One pass: a level chain per series, and an MD5 of each stream's packets (stream copy, no decoding).
    $tail = 'aresample=8000,highpass=f=300,lowpass=f=3400,asetnsamples=n=160:p=0,' +
        'astats=metadata=1:reset=1:measure_perchannel=none:measure_overall=RMS_level,' +
        'ametadata=print:key=lavfi.astats.Overall.RMS_level:file='
    $series = @()
    $parts = @()
    if ($hasFc) {
        $parts += "[0:$Main]asplit=2[m1][m2]"
        $parts += "[m1]aformat=channel_layouts=mono,${tail}raw-main-mix.txt[o0]"
        $parts += "[m2]pan=mono|c0=FC,${tail}raw-main-fc.txt[o1]"
        $series += 'main-mix', 'main-fc'
    } else {
        $parts += "[0:$Main]aformat=channel_layouts=mono,${tail}raw-main-mix.txt[o0]"
        $series += 'main-mix'
    }
    foreach ($n in $Cand) {
        $parts += "[0:$n]aformat=channel_layouts=mono,${tail}raw-cand-$n.txt[o$($series.Count)]"
        $series += "cand-$n"
    }
    $graph = $parts -join ';'
    $nulls = (0..($series.Count - 1) | ForEach-Object { "-map [o$_] -f null o$_" }) -join ' '
    $copies = ($all | ForEach-Object { "-map 0:$_" }) -join ' '
    $started = Get-Date
    Push-Location -LiteralPath $out
    try {
        $log = & cmd /c "`"$FFMPEG`" -v error -nostdin -i `"$file`" -filter_complex `"$graph`" $nulls $copies -c copy -f streamhash -hash md5 streamhash.txt 2>&1"
        $code = $LASTEXITCODE
    } finally { Pop-Location }
    $seconds = [int]((Get-Date) - $started).TotalSeconds
    Set-Content -LiteralPath "$out\ffmpeg.txt" -Value $log -Encoding UTF8 -ErrorAction Stop
    if ($code -ne 0) { throw "ffmpeg failed (exit $code): $(@($log) | Select-Object -First 3)" }

    # streamhash.txt has one line per copied stream, in -map order: "<k>,a,MD5=<hash>".
    $md5 = @{}
    foreach ($line in @(Get-Content -LiteralPath "$out\streamhash.txt" -ErrorAction Stop)) {
        if ($line -match '^(\d+),a,MD5=([0-9a-f]{32})$') { $md5[$all[[int]$Matches[1]]] = $Matches[2] }
    }
    if ($md5.Count -ne $all.Count) { throw "streamhash.txt holds $($md5.Count) of $($all.Count) stream hashes" }

    # The raw ametadata files are about 80 bytes per frame; keep only the compact files.
    $frames = @{}
    foreach ($s in $series) {
        $raw = "$out\raw-$s.txt"
        if (-not (Test-Path -LiteralPath $raw)) { throw "ffmpeg wrote no raw-$s.txt" }
        $header = "# lazymkv audio levels v2: frame_ms=20 band_hz=300-3400 unit=dB floor=-100 series=$s file=$(Get-Word $mkv[0].Name)"
        $frames[$s] = Convert-Levels $raw "$out\$s.txt" $header
        Remove-Item -LiteralPath $raw -Force -ErrorAction Stop
        if ($frames[$s] -eq 0) { throw "$s.txt holds no level" }
    }

    $lines = @('# lazymkv audio-levels v2 (experiment 13)',
        "file=$(Get-Word $mkv[0].Name) bytes=$($mkv[0].Length) seconds=$seconds frame_ms=20 band_hz=300-3400")
    foreach ($n in $all) {
        $s = $info[$n]
        $role = if ($n -eq $Main) { "main=$n" } else { "cand=$n" }
        $count = if ($n -eq $Main) { $frames['main-mix'] } else { $frames["cand-$n"] }
        $fc = if ($n -eq $Main) { ' fc=' + $(if ($hasFc) { 'yes' } else { 'no' }) } else { '' }
        $lines += "$role codec=$(Get-Word $s.codec_name) profile=$(Get-Word $s.profile) channels=$($s.channels) " +
            "layout=$(Get-Word $s.channel_layout) language=$(Get-Word $s.tags.language)$fc frames=$count md5=$($md5[$n])"
    }
    Set-Content -LiteralPath "$out\manifest.txt" -Value $lines -Encoding UTF8 -ErrorAction Stop

    "levels: $($mkv[0].Name) main=$Main cand=$($Cand -join ',') centre=$hasFc ffmpeg_exit=$code seconds=$seconds folder=$name"
    foreach ($n in $all) {
        $s = $info[$n]
        $count = if ($n -eq $Main) { $frames['main-mix'] } else { $frames["cand-$n"] }
        "stream ${n}: $($s.codec_name) $($s.channels) ch $($s.channel_layout) $($s.tags.language) frames=$count md5=$($md5[$n])"
    }
    'RESULT ok'
} catch {
    "RESULT FAILED: $($_.Exception.Message)"
    throw
}
