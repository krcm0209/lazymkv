# Writes the RMS level of every 100 ms of two audio streams of a ripped file, so that a candidate
# track can be compared with the main track by its content (is it the same programme, or does it
# carry speech where the main track is quiet?). Small text files only; nothing is written to the share.
#   main-mix.txt   the main stream, mixed down to mono
#   main-fc.txt    the main stream's centre channel (dialogue); only when it has one
#   cand-mix.txt   the candidate stream, mixed down to mono
#   cand-mid.txt   (left + right) / 2 of the candidate
#   cand-side.txt  (left - right) / 2 of the candidate
param(
    [Parameter(Mandatory)][string]$Slug,
    [Parameter(Mandatory)][string]$Label,
    [Parameter(Mandatory)][int]$Main,
    [Parameter(Mandatory)][int]$Cand,
    [switch]$Force
)
. C:\lazymkv\src\scripts\phase0\env.ps1
try {
    Assert-Slug $Slug
    Assert-Slug $Label
    if (-not $FFMPEG) { throw 'ffmpeg not found' }
    $mkv = @(Get-ChildItem -LiteralPath "$Staging\$Slug\$Label" -Filter *.mkv -ErrorAction Stop)
    if ($mkv.Count -ne 1) { throw "expected one .mkv in $Staging\$Slug\$Label, found $($mkv.Count)" }
    $out = "$Recordings\$Slug\$Label\audio-levels-$Main-$Cand"
    if ((Test-Path -LiteralPath $out) -and -not $Force) { throw "$out already exists and is kept; use -Force to measure again" }
    if (Test-Path -LiteralPath $out) { Remove-Item -LiteralPath $out -Recurse -Force -ErrorAction Stop }
    New-Item -ItemType Directory -Force -Path $out -ErrorAction Stop | Out-Null
    # 8 kHz, frames of 800 samples = 100 ms; one RMS value per frame. File names are relative
    # (the job runs inside $out) because a drive colon cannot be written in a filter argument.
    $tail = 'aresample=8000,asetnsamples=n=800:p=0,astats=metadata=1:reset=1,ametadata=print:key=lavfi.astats.Overall.RMS_level:file='
    $graph = "[0:$Main]asplit=2[m1][m2];" +
        "[m1]aformat=channel_layouts=mono,${tail}main-mix.txt[o1];" +
        "[m2]pan=mono|c0=FC,${tail}main-fc.txt[o2];" +
        "[0:$Cand]asplit=3[c1][c2][c3];" +
        "[c1]aformat=channel_layouts=mono,${tail}cand-mix.txt[o3];" +
        "[c2]pan=mono|c0=0.5*FL+0.5*FR,${tail}cand-mid.txt[o4];" +
        "[c3]pan=mono|c0=0.5*FL-0.5*FR,${tail}cand-side.txt[o5]"
    $started = Get-Date
    Push-Location -LiteralPath $out
    try {
        $log = & cmd /c "`"$FFMPEG`" -v error -nostdin -i `"$($mkv[0].FullName)`" -filter_complex `"$graph`" -map [o1] -f null o1 -map [o2] -f null o2 -map [o3] -f null o3 -map [o4] -f null o4 -map [o5] -f null o5 2>&1"
        $code = $LASTEXITCODE
    } finally { Pop-Location }
    Set-Content -LiteralPath "$out\ffmpeg.txt" -Value $log -Encoding UTF8 -ErrorAction Stop
    $seconds = [int]((Get-Date) - $started).TotalSeconds
    $files = @(Get-ChildItem -LiteralPath $out -Filter *.txt | Where-Object { $_.Name -ne 'ffmpeg.txt' -and $_.Length -gt 0 })
    "levels: $($mkv[0].Name) main=$Main cand=$Cand ffmpeg_exit=$code seconds=$seconds files=$(($files | ForEach-Object { $_.Name + ':' + $_.Length }) -join ',')"
    if ($code -ne 0 -or $files.Count -ne 5) { throw "ffmpeg failed (exit $code, $($files.Count) of 5 level files): $(@($log) | Select-Object -First 3)" }
    'RESULT ok'
} catch {
    "RESULT FAILED: $($_.Exception.Message)"
    throw
}
