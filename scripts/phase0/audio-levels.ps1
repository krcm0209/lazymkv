# Experiment 13 and decision D16. Measures a ripped title's audio streams so that audio_similarity.py can tell a
# commentary, an exact duplicate or programme audio apart by content, and adds the title to the tuning dataset: a
# compact copy of every audio stream, from which later measures can be computed without the disc or the rip.
# One ffmpeg pass over the one .mkv in $Staging\<Slug>\<Label>\ (one read of the file) writes:
#
# In $Recordings\<Slug>\<Label>\audio-levels-20ms-<Main>[-<Cand>...]\ (small text files):
#   manifest.txt   first line "# lazymkv audio-levels v2 (experiment 13)"; then the file, the run time and
#                  frame_ms=20; then one line per measured stream: number, codec, channels, layout (and
#                  layout_guessed=<name> when ffprobe could not name it), language, frames, MD5 of its packets
#                  (identical MD5 = exact duplicate). audio_similarity.py refuses a folder without it, so the first
#                  measurement's folders (audio-levels-<Main>-<Cand>, 100 ms) are never read.
#   main-mix.txt   the main stream mixed down to mono: a header line "# lazymkv audio levels v2: frame_ms=20 ...",
#                  then one level in dB per line, 0.1 dB steps, -100 = silence
#   main-fc.txt    the main stream's centre channel (dialogue); only when its layout has one besides other channels
#   cand-<n>.txt   candidate stream <n> mixed down to mono. Without -Cand (a title with one audio stream, or with one
#                  per language) only the main stream's files are written: the proxy is still made, nothing is judged
#   streams.json   ffprobe's list of the file's streams, with all tags, dispositions, start times, and the duration
#   ffprobe.txt    ffprobe's messages
#   streamhash.txt ffmpeg's MD5 of the packets of every audio stream of the file, in file order
#   graph.txt      the filter graph of the pass (ffmpeg reads it from this file: no length limit, no quoting)
#   command.txt    the ffmpeg command line of the pass
#   ffmpeg.txt     ffmpeg's warnings and errors
#   proxy-check.json  ffprobe's view of the proxy written by this run (only when this run wrote it)
#   audio-proxy.json  a copy of the dataset's manifest of the title's proxy (below). It is written LAST, once the
#                  proxy, its manifest and the copy of this folder are all in the dataset: a folder without it is
#                  from a run that did not finish (or from before D16), and audio_dataset.py counts the title as in
#                  the dataset only when one of its level folders holds it. probe-title.ps1 does not take it for
#                  probe evidence.
# In $TuningData\<Slug>\<Label>\ (the tuning dataset on the share; the folder $TuningData is created when missing):
#   audio-proxy.mka   every audio stream of the file as FLAC, 8 kHz, mono, 24 bit, 18 dB below the level chain's
#                     signal (headroom: ffmpeg does not scale a downmix decoded as float, which can exceed full scale):
#                     one track with the stream's mixdown (what the level chain measures, cut at the same 8 kHz);
#                     when its layout has a centre channel besides others, a second track with that channel; and for a
#                     two-channel stream, a second track with its side signal 0.5*L-0.5*R (the mixdown is 0.5*L+0.5*R,
#                     so the two give the stereo width). Each track is tagged LAZYMKV_SOURCE_STREAM=<ffprobe's stream
#                     number in the rip>, LAZYMKV_SERIES=mix, fc or side, and the stream's language. All tracks share
#                     the rip's timeline. A reader adds the 18 dB back in float and measures in the source stream's
#                     decoded sample format (proxy_levels.py). A track that peaks within 0.1 dB of full scale before
#                     it is stored as integers may be clipped: the run then fails.
#   audio-proxy.json  what was kept and from what: the source file, each track with its source stream's facts, packet
#                     MD5 and peak, the encoding, the time. Written after the proxy: a proxy without it is incomplete.
#   audio-levels-20ms-<Main>[-<Cand>...]\   a copy of the level folder above, with audio-proxy.json in it
# The proxy is made once per rip: a later run on the same file (another language's main stream) keeps it.
# Files on their way to the share end in .partial and are renamed only once checked. A failed run removes the
# .partial files it wrote, and every run removes those an interrupted run left. A finished proxy is never removed by
# a failure: a later run keeps it when its manifest matches the file, and makes it again when the manifest is missing.
# An existing level folder, here or in the dataset, is kept unless -Force is given; one without audio-proxy.json is
# refused with "did not finish", which -Force repeats. -Force also replaces a dataset proxy that was made from another
# file than the one now in staging.
# Whether a stream has a centre channel is read from the installed ffmpeg's own table of channel layouts
# (ffmpeg -layouts), not from a list kept here. A layout that ffmpeg does not name and ffprobe does not spell out
# (for example "unknown", as for LPCM in Matroska) is taken as ffmpeg's guess for that many channels, the layout
# ffmpeg itself mixes such a stream down as (6 channels: 5.1; 8: 7.1); the manifest says layout_guessed=<name>.
# Stream numbers are ffprobe's stream indexes (0 is normally the video).
# The last line is "RESULT ok" or "RESULT FAILED: <reason>"; after a failure the script throws.
param(
    [Parameter(Mandatory)][string]$Slug,
    [Parameter(Mandatory)][string]$Label,
    [Parameter(Mandatory)][int]$Main,
    [int[]]$Cand = @(),
    [switch]$Force
)
. C:\lazymkv\src\scripts\phase0\env.ps1

$Key = 'lavfi.astats.Overall.RMS_level='
$PeakKey = 'lavfi.astats.Overall.Peak_level='
$ProxyFormat = 'lazymkv audio proxy v1'
$ProxyRate = 8000
$ProxyHeadroomDb = 18
$ProxyLevel = 5
$ClipDb = -0.1
# ffmpeg's guess for a stream whose layout it cannot name, by channel count (its default layout for that count;
# ffmpeg 8.1.2 prints "Guessed Channel Layout: 5.1" for six unnamed channels). The names are looked up in ffmpeg's own
# table, so 2.1 (three channels) has no centre channel.
$GuessedLayouts = @{ 1 = 'mono'; 2 = 'stereo'; 3 = '2.1'; 4 = '4.0'; 5 = '5.0'; 6 = '5.1'; 7 = '6.1'; 8 = '7.1' }

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

# The last running peak in an ametadata file of astats (reset=0): the track's peak in dBFS, as text ('-inf' for
# silence); '' when the file holds none.
function Get-LastPeak([string]$Raw) {
    $last = ''
    foreach ($line in [System.IO.File]::ReadLines($Raw)) {
        if ($line.StartsWith($PeakKey)) { $last = $line.Substring($PeakKey.Length).Trim() }
    }
    $last
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

# A stream's channels and, when ffprobe could not name its layout, the layout ffmpeg guesses for its channel count.
function Get-StreamLayout($Stream, $Table) {
    $named = @(Get-LayoutChannels ([string]$Stream.channel_layout) $Table)
    if ($named.Count -gt 0) { return [pscustomobject]@{ channels = $named; guessed = '' } }
    [int]$count = 0
    [void][int]::TryParse([string]$Stream.channels, [ref]$count)
    $guess = $GuessedLayouts[$count]
    if ($guess -and $Table.ContainsKey($guess)) { return [pscustomobject]@{ channels = @($Table[$guess]); guessed = $guess } }
    [pscustomobject]@{ channels = @(); guessed = '' }
}

# A Matroska duration tag ("01:02:03.456000000") in seconds; -1 when there is none (the muxer did not finish).
function Get-TagSeconds([string]$Text) {
    if ($Text -match '^(\d+):(\d\d):(\d\d(\.\d+)?)$') {
        return [int]$Matches[1] * 3600 + [int]$Matches[2] * 60 + [double]::Parse($Matches[3], [Globalization.CultureInfo]::InvariantCulture)
    }
    -1
}

# Text as UTF-8 without a byte order mark: ffmpeg would read a BOM in graph.txt as part of the graph.
function Write-Plain([string]$Path, [string]$Text) {
    [IO.File]::WriteAllText($Path, $Text, (New-Object System.Text.UTF8Encoding($false)))
}

function Remove-IfThere([string]$Path) {
    if ($Path -and (Test-Path -LiteralPath $Path)) { Remove-Item -LiteralPath $Path -Recurse -Force -ErrorAction Stop }
}

# What this run has on the share that is not finished yet (.partial names only); a failure removes these.
$partials = New-Object System.Collections.ArrayList
try {
    Assert-Slug $Slug
    Assert-Slug $Label
    if (-not $FFMPEG) { throw 'ffmpeg not found' }
    if (-not $FFPROBE) { throw 'ffprobe not found' }
    if (-not $TuningData) { throw "`$TuningData is not set (C:\lazymkv\phase0\config.ps1)" }
    $Cand = @($Cand)
    if ($Cand -contains $Main) { throw "stream $Main is the main stream and cannot also be a candidate" }
    if (@($Cand | Select-Object -Unique).Count -ne $Cand.Count) { throw "a candidate stream is listed twice: $($Cand -join ',')" }
    $all = @($Main) + $Cand
    $name = 'audio-levels-20ms-' + ($all -join '-')
    Assert-Slug $name
    $layoutTable = Get-LayoutTable
    $ffmpegVersion = [string](@(& $FFMPEG -hide_banner -version) | Select-Object -First 1)
    if (-not (Test-Path -LiteralPath $Staging)) { throw "cannot reach the staging share $Staging" }
    $dir = "$Staging\$Slug\$Label"
    if (-not (Test-Path -LiteralPath $dir)) { throw "expected one .mkv in $dir, found 0 (the folder does not exist: the rip is not in staging)" }
    $mkv = @(Get-ChildItem -LiteralPath $dir -File -ErrorAction Stop | Where-Object { $_.Extension -eq '.mkv' })
    if ($mkv.Count -ne 1) { throw "expected one .mkv in $dir, found $($mkv.Count)" }
    $file = $mkv[0].FullName
    if (-not (Test-Path -LiteralPath $TuningData)) {
        # The staging share answered, so a missing dataset folder is a first run, not an outage.
        try { New-Item -ItemType Directory -Path $TuningData -ErrorAction Stop | Out-Null }
        catch { throw "cannot reach the tuning dataset $TuningData ($($_.Exception.Message))" }
        "dataset: created $TuningData"
    }
    $rec = "$Recordings\$Slug\$Label"
    $out = "$rec\$name"
    $tune = "$TuningData\$Slug\$Label"
    $tuneLevels = "$tune\$name"
    $proxy = "$tune\audio-proxy.mka"
    $proxyJson = "$tune\audio-proxy.json"
    if ((Test-Path -LiteralPath $out) -and -not $Force) {
        if (Test-Path -LiteralPath "$out\audio-proxy.json") { throw "$out already exists and is kept as evidence; use -Force only where the plan says so" }
        throw "$out is from a run that did not finish (it has no audio-proxy.json, which a finished run writes last), or from before D16: repeat with -Force"
    }
    if ((Test-Path -LiteralPath $tuneLevels) -and -not $Force) { throw "$tuneLevels already exists and is kept as evidence; use -Force only where the plan says so" }
    # The proxy is made once per rip: it is kept when its manifest names this very file and its size matches.
    $writeProxy = $true
    if (Test-Path -LiteralPath $proxyJson) {
        $kept = Get-Content -LiteralPath $proxyJson -Raw -Encoding UTF8 -ErrorAction Stop | ConvertFrom-Json
        $same = ([string]$kept.source.file -eq $mkv[0].Name) -and ([string]$kept.source.bytes -eq [string]$mkv[0].Length)
        if ($same) { $same = (Test-Path -LiteralPath $proxy) }
        if ($same) { $same = ([string](Get-Item -LiteralPath $proxy).Length -eq [string]$kept.proxy.bytes) }
        if ($same) { $writeProxy = $false }
        elseif (-not $Force) {
            throw "$proxyJson describes a proxy of $($kept.source.file) ($($kept.source.bytes) bytes), but staging holds $($mkv[0].Name) ($($mkv[0].Length) bytes), or the proxy file is missing or of another size; use -Force only where the plan says so"
        }
    }

    # From here on, folders are created and changed.
    if ($Force) { Remove-IfThere $out; Remove-IfThere $tuneLevels }
    New-Item -ItemType Directory -Force -Path $out -ErrorAction Stop | Out-Null
    New-Item -ItemType Directory -Force -Path $tune -ErrorAction Stop | Out-Null
    foreach ($p in "$proxy.partial", "$proxyJson.partial", "$tuneLevels.partial") {
        if (Test-Path -LiteralPath $p) { Remove-IfThere $p; "dataset: removed $p, left by an interrupted run" }
    }
    if ($writeProxy) {
        # The manifest goes first: a proxy without it is never taken for a complete one.
        Remove-IfThere $proxyJson
        if (Test-Path -LiteralPath $proxy) { Remove-IfThere $proxy; "dataset: removed $proxy (it had no manifest, or was made from another file)" }
    }

    # Which streams the file has. File names are relative (the tools run inside $out): a drive colon cannot be
    # written in a filter argument.
    Push-Location -LiteralPath $out
    try {
        $probeLog = & cmd /c "`"$FFPROBE`" -v error -show_entries stream=index,codec_type,codec_name,profile,channels,channel_layout,sample_rate,sample_fmt,bits_per_raw_sample,start_time,duration:stream_tags:stream_disposition:format=duration -of json -o streams.json `"$file`" 2>&1"
        $code = $LASTEXITCODE
    } finally { Pop-Location }
    Set-Content -LiteralPath "$out\ffprobe.txt" -Value $probeLog -Encoding UTF8 -ErrorAction Stop
    if ($code -ne 0 -or -not (Test-Path -LiteralPath "$out\streams.json")) { throw "ffprobe failed (exit $code): $(@($probeLog) | Select-Object -First 3)" }
    $probe = Get-Content -LiteralPath "$out\streams.json" -Raw -Encoding UTF8 -ErrorAction Stop | ConvertFrom-Json
    $streams = @($probe.streams)
    foreach ($n in $all) {
        $s = @($streams | Where-Object { $_.index -eq $n }) | Select-Object -First 1
        if (-not $s) { throw "the file has no stream $n" }
        if ($s.codec_type -ne 'audio') { throw "stream $n is $($s.codec_type), not audio" }
    }
    $info = @{}
    foreach ($s in $streams) { if ($s.codec_type -eq 'audio') { $info[[int]$s.index] = $s } }
    $audio = @($info.Keys | Sort-Object)
    # A centre channel besides other channels, per audio stream: for mono the centre is the mixdown itself.
    $hasFc = @{}
    $guessed = @{}
    foreach ($n in $audio) {
        $lay = Get-StreamLayout $info[$n] $layoutTable
        $hasFc[$n] = (@($lay.channels) -contains 'FC') -and (@($lay.channels).Count -gt 1)
        $guessed[$n] = [string]$lay.guessed
    }
    $mainFc = $hasFc[$Main]

    # One pass. Per audio stream: the level chains it is measured by (main or candidate) and, when the proxy is
    # made, its proxy tracks; asplit hands the same decoded audio to each. The packets of every audio stream are
    # copied, without decoding, into ffmpeg's streamhash muxer.
    $tail = 'aresample=8000,highpass=f=300,lowpass=f=3400,asetnsamples=n=160:p=0,' +
        'astats=metadata=1:reset=1:measure_perchannel=none:measure_overall=RMS_level,' +
        'ametadata=print:key=lavfi.astats.Overall.RMS_level:file='
    # The proxy chain: the same resampler as the level chain, the headroom, then the running peak of the track (in
    # float, before it is stored as integers) into peak-<k>.txt, one value per second.
    $toProxy = "aresample=$ProxyRate,volume=-${ProxyHeadroomDb}dB,asetnsamples=n=${ProxyRate}:p=0," +
        'astats=metadata=1:reset=0:measure_perchannel=none:measure_overall=Peak_level,' +
        'ametadata=print:key=lavfi.astats.Overall.Peak_level:file='
    $series = @()
    $tracks = @()
    $parts = @()
    foreach ($n in $audio) {
        $branches = @()
        if ($n -eq $Main) {
            $branches += "aformat=channel_layouts=mono,${tail}raw-main-mix.txt[o$($series.Count)]"
            $series += 'main-mix'
            if ($mainFc) {
                $branches += "pan=mono|c0=FC,${tail}raw-main-fc.txt[o$($series.Count)]"
                $series += 'main-fc'
            }
        } elseif ($Cand -contains $n) {
            $branches += "aformat=channel_layouts=mono,${tail}raw-cand-$n.txt[o$($series.Count)]"
            $series += "cand-$n"
        }
        if ($writeProxy) {
            $k = $tracks.Count
            $branches += "aformat=channel_layouts=mono,${toProxy}peak-$k.txt,aformat=sample_fmts=s32[p$k]"
            $tracks += [pscustomobject]@{ stream = $n; series = 'mix' }
            if ($hasFc[$n]) {
                $k = $tracks.Count
                $branches += "pan=mono|c0=FC,${toProxy}peak-$k.txt,aformat=sample_fmts=s32[p$k]"
                $tracks += [pscustomobject]@{ stream = $n; series = 'fc' }
            }
            if ([string]$info[$n].channels -eq '2') {
                # By channel number, not name: it works whatever ffprobe calls the two channels.
                $k = $tracks.Count
                $branches += "pan=mono|c0=0.5*c0-0.5*c1,${toProxy}peak-$k.txt,aformat=sample_fmts=s32[p$k]"
                $tracks += [pscustomobject]@{ stream = $n; series = 'side' }
            }
        }
        if ($branches.Count -eq 1) {
            $parts += "[0:$n]$($branches[0])"
        } elseif ($branches.Count -gt 1) {
            $labels = (0..($branches.Count - 1) | ForEach-Object { "[s${n}b$_]" }) -join ''
            $parts += "[0:$n]asplit=$($branches.Count)$labels"
            for ($b = 0; $b -lt $branches.Count; $b++) { $parts += "[s${n}b$b]$($branches[$b])" }
        }
    }
    Write-Plain "$out\graph.txt" ($parts -join ';')
    $nulls = (0..($series.Count - 1) | ForEach-Object { "-map [o$_] -f null o$_" }) -join ' '
    $hashes = ($audio | ForEach-Object { "-map 0:$_" }) -join ' '
    $partialProxy = "$proxy.partial"
    $proxyArgs = ''
    if ($writeProxy) {
        $meta = @()
        for ($k = 0; $k -lt $tracks.Count; $k++) {
            $lang = [string]$info[$tracks[$k].stream].tags.language
            if ($lang -cnotmatch '^[a-z]{3}$') { $lang = 'und' }
            $meta += "-metadata:s:a:$k LAZYMKV_SOURCE_STREAM=$($tracks[$k].stream) -metadata:s:a:$k LAZYMKV_SERIES=$($tracks[$k].series) -metadata:s:a:$k language=$lang"
        }
        $maps = (0..($tracks.Count - 1) | ForEach-Object { "-map [p$_]" }) -join ' '
        $proxyArgs = " $maps -c:a flac -compression_level $ProxyLevel -bits_per_raw_sample 24 $($meta -join ' ') -f matroska `"$partialProxy`""
    }
    $command = "`"$FFMPEG`" -v warning -nostats -nostdin -i `"$file`" -/filter_complex graph.txt $nulls $hashes -c copy -f streamhash -hash md5 streamhash.txt$proxyArgs"
    Set-Content -LiteralPath "$out\command.txt" -Value $command -Encoding UTF8 -ErrorAction Stop
    if ($command.Length -gt 8000) { throw "the ffmpeg command line is $($command.Length) characters long, more than cmd.exe takes (8191); the file has $($audio.Count) audio streams" }
    if ($writeProxy) { [void]$partials.Add($partialProxy) }
    $started = Get-Date
    Push-Location -LiteralPath $out
    try {
        $log = & cmd /c "$command 2>&1"
        $ffmpegExit = $LASTEXITCODE
    } finally { Pop-Location }
    $seconds = [int]((Get-Date) - $started).TotalSeconds
    Set-Content -LiteralPath "$out\ffmpeg.txt" -Value $log -Encoding UTF8 -ErrorAction Stop
    if ($ffmpegExit -ne 0) { throw "ffmpeg failed (exit $ffmpegExit): $(@($log) | Select-Object -Last 3)" }

    # streamhash.txt has one line per copied stream, in -map order (every audio stream, in file order): "<k>,a,MD5=<hash>".
    $md5 = @{}
    foreach ($line in @(Get-Content -LiteralPath "$out\streamhash.txt" -ErrorAction Stop)) {
        if ($line -match '^(\d+),a,MD5=([0-9a-f]{32})$') { $md5[$audio[[int]$Matches[1]]] = $Matches[2] }
    }
    if ($md5.Count -ne $audio.Count) { throw "streamhash.txt holds $($md5.Count) of $($audio.Count) stream hashes" }

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
        $fc = if ($n -eq $Main) { ' fc=' + $(if ($mainFc) { 'yes' } else { 'no' }) } else { '' }
        $guess = if ($guessed[$n]) { " layout_guessed=$($guessed[$n])" } else { '' }
        $lines += "$role codec=$(Get-Word $s.codec_name) profile=$(Get-Word $s.profile) channels=$($s.channels) " +
            "layout=$(Get-Word $s.channel_layout)$guess language=$(Get-Word $s.tags.language)$fc frames=$count md5=$($md5[$n])"
    }
    Set-Content -LiteralPath "$out\manifest.txt" -Value $lines -Encoding UTF8 -ErrorAction Stop

    # The proxy: checked (readable, one FLAC 8 kHz mono track per expected source and series, each with the
    # duration the muxer writes when it finishes, none at full scale), renamed, then described by its manifest.
    if ($writeProxy) {
        if (-not (Test-Path -LiteralPath $partialProxy)) { throw "proxy check failed: ffmpeg wrote no $partialProxy" }
        $proxyBytes = (Get-Item -LiteralPath $partialProxy).Length
        if ($proxyBytes -eq 0) { throw "proxy check failed: $partialProxy is empty" }
        Push-Location -LiteralPath $out
        try {
            $checkLog = & cmd /c "`"$FFPROBE`" -v error -show_entries stream=index,codec_type,codec_name,sample_rate,channels,sample_fmt,bits_per_raw_sample:stream_tags -of json -o proxy-check.json `"$partialProxy`" 2>&1"
            $code = $LASTEXITCODE
        } finally { Pop-Location }
        if ($code -ne 0 -or -not (Test-Path -LiteralPath "$out\proxy-check.json")) { throw "proxy check failed: ffprobe cannot read $partialProxy (exit $code): $(@($checkLog) | Select-Object -First 3)" }
        $got = @((Get-Content -LiteralPath "$out\proxy-check.json" -Raw -Encoding UTF8 -ErrorAction Stop | ConvertFrom-Json).streams)
        if ($got.Count -ne $tracks.Count) { throw "proxy check failed: the proxy has $($got.Count) tracks, $($tracks.Count) were written" }
        $trackInfo = New-Object System.Collections.ArrayList
        for ($k = 0; $k -lt $tracks.Count; $k++) {
            $g = $got[$k]
            $t = $tracks[$k]
            $where = "proxy track $k (stream $($t.stream) $($t.series))"
            if ($g.codec_type -ne 'audio' -or $g.codec_name -ne 'flac' -or [string]$g.sample_rate -ne [string]$ProxyRate -or [string]$g.channels -ne '1' -or [string]$g.bits_per_raw_sample -ne '24') {
                throw "proxy check failed: $where is $($g.codec_type) $($g.codec_name) $($g.sample_rate) Hz $($g.channels) ch $($g.bits_per_raw_sample) bit, not flac $ProxyRate Hz 1 ch 24 bit"
            }
            if ([string]$g.tags.LAZYMKV_SOURCE_STREAM -ne [string]$t.stream -or [string]$g.tags.LAZYMKV_SERIES -ne $t.series) {
                throw "proxy check failed: $where is tagged stream $($g.tags.LAZYMKV_SOURCE_STREAM) $($g.tags.LAZYMKV_SERIES)"
            }
            $duration = Get-TagSeconds ([string]$g.tags.DURATION)
            if ($duration -le 0) { throw "proxy check failed: $where has no duration (the file was not finished)" }
            $peakFile = "$out\peak-$k.txt"
            if (-not (Test-Path -LiteralPath $peakFile)) { throw "proxy check failed: ffmpeg wrote no peak-$k.txt for $where" }
            $peak = Get-LastPeak $peakFile
            if (-not $peak) { throw "proxy check failed: peak-$k.txt holds no peak for $where" }
            if (-not ($peak.Contains('inf') -or $peak.Contains('nan'))) {
                if ([double]::Parse($peak, [Globalization.CultureInfo]::InvariantCulture) -ge $ClipDb) {
                    throw "proxy check failed: $where peaks at $peak dBFS after the $ProxyHeadroomDb dB of headroom, at full scale: it may be clipped"
                }
            }
            Remove-Item -LiteralPath $peakFile -Force -ErrorAction Stop
            $s = $info[$t.stream]
            [void]$trackInfo.Add([ordered]@{
                track = $k; source_stream = $t.stream; series = $t.series; duration_s = [math]::Round($duration, 3); peak_dbfs = $peak
                source = [ordered]@{
                    codec = [string]$s.codec_name; profile = [string]$s.profile; channels = [int]$s.channels
                    layout = [string]$s.channel_layout; layout_guessed = $guessed[$t.stream]; language = [string]$s.tags.language; title = [string]$s.tags.title
                    sample_rate = [string]$s.sample_rate; sample_fmt = [string]$s.sample_fmt; bits_per_raw_sample = [string]$s.bits_per_raw_sample
                    start_time = [string]$s.start_time; md5 = $md5[$t.stream]
                }
            })
        }
        # From here on a failure leaves the finished proxy in place: it is not in $partials.
        Move-Item -LiteralPath $partialProxy -Destination $proxy -ErrorAction Stop
        [void]$partials.Remove($partialProxy)
        $manifest = [ordered]@{
            format = $ProxyFormat; slug = $Slug; label = $Label; created = $started.ToString('yyyy-MM-dd HH:mm:ss')
            source = [ordered]@{
                file = $mkv[0].Name; bytes = $mkv[0].Length; modified = $mkv[0].LastWriteTime.ToString('yyyy-MM-dd HH:mm:ss')
                duration_s = [string]$probe.format.duration; audio_streams = $audio.Count
            }
            pass = [ordered]@{ seconds = $seconds; ffmpeg = $ffmpegVersion; levels_folder = $name }
            encoding = [ordered]@{
                codec = 'flac'; sample_rate = $ProxyRate; channels = 1; bits = 24; compression_level = $ProxyLevel
                gain_db = -$ProxyHeadroomDb
                mix = 'aformat=channel_layouts=mono on the decoded stream, as in the level chain'
                fc = 'pan=mono|c0=FC, for every stream whose layout has FC besides other channels'
                side = 'pan=mono|c0=0.5*c0-0.5*c1, for every two-channel stream (the mix is 0.5*c0+0.5*c1)'
                chain = "aresample=$ProxyRate,volume=-${ProxyHeadroomDb}dB,aformat=sample_fmts=s32"
                read = "add $ProxyHeadroomDb dB in float, convert to the source stream's sample_fmt, then measure (proxy_levels.py)"
            }
            proxy = [ordered]@{ file = 'audio-proxy.mka'; bytes = $proxyBytes; tracks = $tracks.Count }
            not_kept = 'video and subtitle streams; channels other than the mixdown, the centre and a stereo pair''s side signal; sound above 4 kHz; object-audio metadata; the packets themselves (their MD5 is kept)'
            tracks = $trackInfo.ToArray()
        }
        $json = $manifest | ConvertTo-Json -Depth 6
        [void]$partials.Add("$proxyJson.partial")
        Set-Content -LiteralPath "$proxyJson.partial" -Value $json -Encoding UTF8 -ErrorAction Stop
        Move-Item -LiteralPath "$proxyJson.partial" -Destination $proxyJson -ErrorAction Stop
        $partials.Clear()
        $proxyLine = "proxy: $proxy tracks=$($tracks.Count) bytes=$proxyBytes (" + (($tracks | ForEach-Object { "$($_.stream) $($_.series)" }) -join ', ') + ')'
    } else {
        $json = Get-Content -LiteralPath $proxyJson -Raw -Encoding UTF8 -ErrorAction Stop
        $proxyLine = "proxy: kept $proxy (made by an earlier run from this same file)"
    }

    # The level folder joins the dataset: copied under a .partial name, compared, then renamed.
    $copy = "$tuneLevels.partial"
    [void]$partials.Add($copy)
    Copy-Item -LiteralPath $out -Destination $copy -Recurse -ErrorAction Stop
    $here = @(Get-ChildItem -LiteralPath $out -File -ErrorAction Stop | Sort-Object Name | ForEach-Object { "$($_.Name)=$($_.Length)" }) -join ' '
    $there = @(Get-ChildItem -LiteralPath $copy -File -ErrorAction Stop | Sort-Object Name | ForEach-Object { "$($_.Name)=$($_.Length)" }) -join ' '
    if ($here -ne $there) { throw "the copy of $name in the dataset differs from $out (here: $here; there: $there)" }
    Rename-Item -LiteralPath $copy -NewName $name -ErrorAction Stop
    $partials.Clear()
    Set-Content -LiteralPath "$tuneLevels\audio-proxy.json" -Value $json -Encoding UTF8 -ErrorAction Stop
    # Last: this file marks the title as complete in the dataset (audio_dataset.py, and the refusal above).
    Set-Content -LiteralPath "$out\audio-proxy.json" -Value $json -Encoding UTF8 -ErrorAction Stop

    "levels: $($mkv[0].Name) main=$Main cand=$($Cand -join ',') centre=$mainFc ffmpeg_exit=$ffmpegExit seconds=$seconds folder=$name"
    foreach ($n in $all) {
        $s = $info[$n]
        $count = if ($n -eq $Main) { $frames['main-mix'] } else { $frames["cand-$n"] }
        $guess = if ($guessed[$n]) { " (layout guessed by ffmpeg: $($guessed[$n]))" } else { '' }
        "stream ${n}: $($s.codec_name) $($s.channels) ch $($s.channel_layout) $($s.tags.language) frames=$count md5=$($md5[$n])$guess"
    }
    $proxyLine
    "dataset: $tuneLevels"
    'RESULT ok'
} catch {
    $failure = $_
    foreach ($p in @($partials)) { try { Remove-IfThere $p } catch { } }
    "RESULT FAILED: $($failure.Exception.Message)"
    throw $failure
}
