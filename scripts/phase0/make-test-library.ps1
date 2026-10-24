# Experiment 9: generated files named as spec 8.9 prescribes, for the owner to add to Plex and Jellyfin.
# Run as a job (the share is reachable only from one):
#   scripts/phase0/job.sh make-test-library.ps1 exp09-<run> [-Style combined|plex|jellyfin] [-Compat] [-Force]
# <run> is the style, with "-compat" added when -Compat is given; it names the log and the summary.
# -Compat adds the " [tmdbid-ID]" tag that Jellyfin 10.x needs (spec 8.9, jellyfin10_compat).
# $TestLibrary must be an existing, empty folder that the owner made for this test, written as
# \\unas-pro\media\<folder>[\<folder>...] and outside \\unas-pro\media\Movies and \\unas-pro\media\Shows.
# The script never creates it, never overwrites a file and never deletes anything in it. Both clips are made
# on C: before the first copy, so a failed encode leaves the library untouched; the clips are removed when the
# script ends.
# The summary $P0\exp09\summary-<run>.json is evidence: an existing one is refused unless -Force is given.
# The last line is "RESULT ok" or "RESULT FAILED: <reason>"; after a failure the script throws.
param(
    [string]$Style = 'combined',
    [switch]$Compat,
    [switch]$Force
)
. C:\lazymkv\src\scripts\phase0\env.ps1

# One 8-second clip of the given size. ffmpeg's exit code and the output file are both checked.
function New-Clip([string]$Size, [string]$Out) {
    & $FFMPEG -nostdin -v error -y -f lavfi -i "testsrc2=size=${Size}:rate=24" -f lavfi -i 'sine=frequency=440:sample_rate=48000' -t 8 -c:v libx264 -preset ultrafast -crf 30 -pix_fmt yuv420p -c:a aac $Out | Out-Null
    $code = $LASTEXITCODE
    if ($code -ne 0 -or -not (Test-Path -LiteralPath $Out) -or (Get-Item -LiteralPath $Out).Length -eq 0) { throw "ffmpeg did not write $Out (exit $code)" }
}

# .NET calls: a bracket in a name ([tmdbid-...] with -Compat) is never read as a wildcard, File.Copy never
# overwrites, and a failure is always a terminating error.
function Place([string]$Source, [string]$Relative) {
    $destination = [System.IO.Path]::Combine($root, $Relative)
    [System.IO.Directory]::CreateDirectory([System.IO.Path]::GetDirectoryName($destination)) | Out-Null
    [System.IO.File]::Copy($Source, $destination, $false)
    $length = (Get-Item -LiteralPath $destination -ErrorAction Stop).Length
    if ($length -ne (Get-Item -LiteralPath $Source -ErrorAction Stop).Length) { throw "short copy: $destination" }
    [void]$placed.Add([pscustomobject]@{ path = $destination; bytes = $length })
}

function Tagged([string]$Title, [int]$TmdbId) {
    if ($Compat) { "$Title {tmdb-$TmdbId} [tmdbid-$TmdbId]" } else { "$Title {tmdb-$TmdbId}" }
}

$failure = $null
$summary = $null
$placed = New-Object System.Collections.ArrayList
$tmp = "$P0\tmp\exp09"
try {
    if (@('combined', 'plex', 'jellyfin') -cnotcontains $Style) { throw "-Style must be combined, plex or jellyfin, not '$Style'" }
    $run = $Style
    if ($Compat) { $run = "$Style-compat" }
    Assert-Slug $run
    if (-not $FFMPEG) { throw 'ffmpeg.exe not found' }

    # The library is checked before anything is created, copied or deleted.
    if (-not $TestLibrary) { throw 'Set $TestLibrary in C:\lazymkv\phase0\config.ps1 first.' }
    $root = $TestLibrary.TrimEnd('\')
    # Nothing may be written into Movies or Shows (owner's rule). A plain string comparison misses another name for the
    # same place (an IP address, unas-pro.local, a mapped drive letter, \\?\UNC\..., a "/", a "." or ".." segment,
    # a name Windows trims), so only the one form this check can judge is accepted. -match ignores case.
    if ($root -notmatch '^\\\\unas-pro\\media(\\[A-Za-z0-9_-]+)+$' -or $root -match '^\\\\unas-pro\\media\\(Movies|Shows)(\\|$)') {
        throw "$root is not a folder of the form \\unas-pro\media\<folder> outside Movies and Shows (names of letters, digits, - and _ only); give the test folder by that path"
    }
    if (-not (Test-Path -LiteralPath $root -PathType Container)) { throw "$root does not exist or cannot be reached; the owner creates the empty test folder first" }
    $entries = @(Get-ChildItem -LiteralPath $root -Force -ErrorAction Stop)
    $seen = 'none'
    if ($entries.Count -gt 0) { $seen = ($entries | Select-Object -First 10 | ForEach-Object { $_.Name }) -join ', ' }
    "library: $root ($($entries.Count) entries, hidden ones included): $seen"
    $ignore = '.DS_Store', 'Thumbs.db', 'desktop.ini', '@eaDir', '#recycle', '$RECYCLE.BIN', 'System Volume Information'
    $present = @($entries | Where-Object { $ignore -notcontains $_.Name } | ForEach-Object { $_.Name })
    if ($present.Count -gt 0) {
        $shown = ($present | Select-Object -First 5) -join ', '
        throw "$root is not empty (found: $shown); it must be the empty folder the owner made for this test"
    }
    $expDir = "$P0\exp09"
    $summaryFile = "$expDir\summary-$run.json"
    if ((Test-Path -LiteralPath $summaryFile) -and -not $Force) {
        throw "$summaryFile already exists and is kept as evidence; use -Force only where the plan says so"
    }

    New-Item -ItemType Directory -Force -Path $expDir -ErrorAction Stop | Out-Null
    New-Item -ItemType Directory -Force -Path $tmp -ErrorAction Stop | Out-Null
    $summary = [ordered]@{
        ok = $false; result = $null; run = $run; style = $Style; compat = [bool]$Compat; library = $root
        started = (Get-Date).ToString('yyyy-MM-dd HH:mm:ss'); files = @()
    }

    # Both clips first: nothing is copied to the share unless both exist.
    New-Clip '3840x2160' "$tmp\2160.mkv"
    New-Clip '1920x1080' "$tmp\1080.mkv"

    $alien = Tagged 'Alien (1979)' 348
    $inception = Tagged 'Inception (2010)' 27205
    $blade = Tagged 'Blade Runner (1982)' 78

    # A lone file with a resolution label.
    Place "$tmp\2160.mkv" "$alien\$alien - 2160p.mkv"
    # Two resolutions of one film: versions (D11).
    Place "$tmp\2160.mkv" "$inception\$inception - 2160p.mkv"
    Place "$tmp\1080.mkv" "$inception\$inception - 1080p.mkv"
    # Two cuts in the chosen edition style (spec 8.9).
    foreach ($cut in 'Theatrical Cut', 'Final Cut') {
        switch ($Style) {
            'combined' { Place "$tmp\2160.mkv" "$blade\$blade - 2160p $cut {edition-$cut}.mkv" }
            'plex'     { Place "$tmp\2160.mkv" "$blade {edition-$cut}\$blade {edition-$cut} - 2160p.mkv" }
            'jellyfin' { Place "$tmp\2160.mkv" "$blade\$blade - 2160p $cut.mkv" }
        }
    }
} catch {
    $failure = $_.Exception.Message
}

Remove-Item -LiteralPath "$tmp\2160.mkv", "$tmp\1080.mkv" -Force -ErrorAction SilentlyContinue
if ($null -ne $summary) {
    try {
        foreach ($file in $placed) { '{0}  {1:N0} bytes' -f $file.path, $file.bytes }
        $summary['files'] = @($placed | ForEach-Object { [ordered]@{ path = $_.path.Substring($root.Length + 1); bytes = $_.bytes } })
        if (-not $failure -and $placed.Count -ne 5) { $failure = "expected 5 files, placed $($placed.Count)" }
    } catch {
        if (-not $failure) { $failure = "listing the files failed: $($_.Exception.Message)" }
    }
    $summary['ok'] = -not $failure
    $summary['result'] = if ($failure) { "FAILED: $failure" } else { 'ok' }
    $json = $summary | ConvertTo-Json -Depth 4
    try { Set-Content -LiteralPath $summaryFile -Value $json -Encoding UTF8 -ErrorAction Stop } catch { if (-not $failure) { $failure = "cannot write ${summaryFile}: $($_.Exception.Message)" } }
    $json
}
if ($failure) {
    "RESULT FAILED: $failure"
    throw "make-test-library.ps1: $failure"
}
'RESULT ok'
