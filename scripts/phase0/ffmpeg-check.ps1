# The one-off check of a minimal FFmpeg build on the mini PC (design part 11.3; pull request 6's notes, call 12 (a);
# Rulings T6-R12, T6-R13 and T6-R14). It runs in the owner's desktop session, since it reads the staging share, so it
# lives under scripts\phase0\, the only folder run-interactive.ps1 runs, and its output is the log
# C:\lazymkv\phase0\logs\<log name>.txt. Build first over SSH (scripts/box.sh "cargo build --release --locked
# -p lazymkv-dev -p lazymkv-capture"), and run no scripts/box.sh until the job ends: each sync replaces C:\lazymkv\src.
# Usage: scripts/phase0/job.sh ffmpeg-check.ps1 <log name> -Self [-Evidence <dir>] [-Work <dir>]
#        scripts/phase0/job.sh ffmpeg-check.ps1 <log name> -FfmpegSha256 <hex> -FfprobeSha256 <hex>
#            [-Candidate <dir>] [-Evidence <dir>] [-Work <dir>]
# job.sh passes the arguments unquoted, so no value may hold a space or a quote; every path has a default.
# -Self checks the FFmpeg that env.ps1 resolves (the installed build) and leaves PATH alone (T6-R14). The other form
# checks the two programs in -Candidate, which must have the two SHA-256 (Secure Hash Algorithm 256) values given; that
# folder goes first on PATH before env.ps1 is dot-sourced, so this script, audio-levels.ps1 and lazymkv-dev all use it.
# The film is phase 0's kept UHD (Ultra HD Blu-ray) all-tracks rip: the one recording whose folder name ends -uhd and
# whose all-tracks folder holds at least one finished level folder (audio-levels-20ms-<main>[-<cand>...] with
# audio-proxy.json); none or several such recordings is a failure. The check repeats phase 0's sequence of level
# passes (controller ruling T6-E15): one audio-levels.ps1 pass per finished level folder, all into the same scratch
# roots, so the first pass writes the proxy and the later ones find it and keep it, as phase 0's did. The first is the
# one folder that holds proxy-check.json, which audio-levels.ps1 writes only in a pass that writes the proxy; the
# others follow by the time of their manifest.txt, then by name. Only the first matters: a pass that keeps the proxy
# reads nothing else that an earlier pass wrote, since each writes its own level folder. All the folders must share
# one main stream. The passes are named on the levels line by their stream numbers, which are not names.
# The parts, each one output line, in this order (T6-R13):
#   hashes         the candidate's two SHA-256 values against the ones given; nothing else runs unless they are equal
#   levels         phase 0's audio-levels.ps1 passes over the kept rip with -RecordingsRoot and -TuningRoot under
#                  -Work (T6-R12); in each pass's folder every series file, streamhash.txt and graph.txt byte for byte
#                  against phase 0's folder of the same name, and manifest.txt byte for byte with the run time
#                  (seconds=) of its second line left out; the counts are summed and each word is equal only if
#                  equal in every folder
#   proxy          the installed full ffmpeg's packet hashes (streamhash) of the new proxy against phase 0's proxy
#   listing        the candidate ffprobe's stream listing of the kept rip, key by key, against the recording's
#                  ffprobe-streams.json: no key of an audio stream may differ; other streams' differing keys are named
#                  by stream kind and key path, never by value, and do not fail the check
#   packets row    the candidate ffprobe's packet listing of each trial row's evidence.mks, byte for byte against the
#                  recording's subs\sub-<n>.packets.json (six lines, rows from C:\lazymkv\phase0\trial-tracks.txt)
#   corpus copy    the whole of -Evidence copied to <Work>\evidence; refused above 4 GB
#   golden check   lazymkv-dev golden check and lazymkv-dev retune over the copy, before and after the film's new
#   retune         listing and level folders are put into it (the listing whole; of each level folder the measured
#                  files only, Set-NewLevelFiles below): the two standard output captures of each and the two
#                  standard error captures of each must be the same bytes, with the same exit code; a difference is
#                  reported in line numbers only. Both run in C:\lazymkv\src, since lazymkv-dev finds the spec from
#                  the repository root above its current folder. The run before must also show that the command read
#                  the copy, not refused it: its standard output is not empty, and its exit code is 0 for golden check
#                  (all pass; 1 is a golden that fails, 3 nothing checked and 2 an argument error, golden.rs) and
#                  0, 1 or 3 for retune (0 pass, 1 a FAIL, 3 something not
#                  checked; 2 an argument or spec error; retune.rs). retune's refusals (no evidence, a TMDb copy more
#                  than six months old that cannot be fetched again) exit 3 with nothing on standard output, so the
#                  empty capture tells them apart from a measured exit 3.
# Every listing and both commands' output are captured as bytes with lazymkv-capture record --out, never through a
# PowerShell pipeline. A part that cannot run without an earlier one prints "<part>: not run".
# The script prints numbers, equal or different, key names, stream kinds and row numbers only: no name of a film, a
# track, a share or a path. It reads the recordings, the tuning dataset, the staging share and the evidence mirror and
# writes only below -Work, which must not exist; the script creates it and always removes it.
# The last line is "RESULT ok" or "RESULT FAILED: <reason>"; after a failure the script throws (wait.sh exits 1).
param(
    [switch]$Self,
    [string]$FfmpegSha256 = '',
    [string]$FfprobeSha256 = '',
    [string]$Candidate = 'C:\lazymkv\tmp\pr6\ffmpeg',
    [string]$Evidence = 'C:\lazymkv\evidence\owner',
    [string]$Work = 'C:\lazymkv\tmp\pr6\check'
)
$ErrorActionPreference = 'Stop'
# Add-Failure, Stop-Part, Get-Reason, Invoke-Recorded and Get-TrialRows, one copy shared with worker-check.ps1 (the
# review of pull request #21 (X13)).
. C:\lazymkv\src\scripts\phase0\check-common.ps1

$DEV = 'C:\lazymkv\target\release\lazymkv-dev.exe'
$LevelsScript = 'C:\lazymkv\src\scripts\phase0\audio-levels.ps1'
$Spec = 'C:\lazymkv\src\docs\superpowers\specs\2026-10-04-lazymkv-design.md'
$Label = 'all-tracks'
$RepoRoot = 'C:\lazymkv\src'
$MaxCopyBytes = 4GB
$Utf8 = New-Object System.Text.UTF8Encoding($false)

$failures = New-Object System.Collections.ArrayList

function Get-Same([bool]$Same) { if ($Same) { 'equal' } else { 'different' } }

# Two files with the same bytes; false when either is missing.
function Test-SameBytes([string]$A, [string]$B) {
    if (-not (Test-Path -LiteralPath $A -PathType Leaf)) { return $false }
    if (-not (Test-Path -LiteralPath $B -PathType Leaf)) { return $false }
    if ((Get-Item -LiteralPath $A).Length -ne (Get-Item -LiteralPath $B).Length) { return $false }
    (Get-FileHash -LiteralPath $A -Algorithm SHA256).Hash -eq (Get-FileHash -LiteralPath $B -Algorithm SHA256).Hash
}

# A JSON (JavaScript Object Notation) file read from its bytes as UTF-8, with or without a byte order mark.
function Read-Json([string]$Path) {
    $text = $Utf8.GetString([IO.File]::ReadAllBytes($Path))
    if ($text.Length -gt 0 -and $text[0] -eq [char]0xFEFF) { $text = $text.Substring(1) }
    $text | ConvertFrom-Json
}

# A file's bytes as text, one character per byte (ISO 8859-1, so no byte is lost or changed and line ends stay as
# they are), and back.
$Latin1 = [Text.Encoding]::GetEncoding(28591)
function Read-Bytes([string]$Path) { $Latin1.GetString([IO.File]::ReadAllBytes($Path)) }

# A manifest's text with the run time of its second line (" seconds=<n> ") set to $Value; nothing else changes.
function Set-RunTime([string]$Text, [string]$Value) {
    $first = $Text.IndexOf("`n")
    if ($first -lt 0) { return $Text }
    $second = $Text.IndexOf("`n", $first + 1)
    if ($second -lt 0) { $second = $Text.Length }
    $line = $Text.Substring($first + 1, $second - $first - 1) -replace ' seconds=\d+ ', " seconds=$Value "
    $Text.Substring(0, $first + 1) + $line + $Text.Substring($second)
}

# manifest.txt with only its run time masked (T6-R13).
function Get-ManifestText([string]$Path) { Set-RunTime (Read-Bytes $Path) '-' }

# The run time on a manifest's second line, or '' when it has none.
function Get-RunTime([string]$Path) {
    $m = [regex]::Match((Read-Bytes $Path), '\A[^\n]*\n[^\n]* seconds=(\d+) ')
    if ($m.Success) { return $m.Groups[1].Value }
    ''
}

# Where two captures first differ, in numbers only: their line counts, how many lines differ and the first that does.
function Get-LineDiff([string]$A, [string]$B) {
    $one = @((Read-Bytes $A) -split "`n")
    $two = @((Read-Bytes $B) -split "`n")
    $count = [math]::Max($one.Count, $two.Count)
    $differ = 0
    $first = 0
    for ($i = 0; $i -lt $count; $i++) {
        if ($i -ge $one.Count -or $i -ge $two.Count -or $one[$i] -cne $two[$i]) {
            $differ++
            if ($first -eq 0) { $first = $i + 1 }
        }
    }
    "$($one.Count) lines before, $($two.Count) after, $differ lines differ, the first at line $first"
}

function Test-SameManifest([string]$A, [string]$B) {
    if (-not (Test-Path -LiteralPath $A -PathType Leaf)) { return $false }
    if (-not (Test-Path -LiteralPath $B -PathType Leaf)) { return $false }
    (Get-ManifestText $A) -ceq (Get-ManifestText $B)
}

# A key path as it may be printed: letters, digits and _ . [ ] - only; any other key, which a file's own tag could
# name, prints as "(unnamed key)".
function Get-KeyName([string]$Key) {
    if ($Key -ceq '(the whole stream)' -or $Key -cmatch '^[A-Za-z0-9_.\[\]-]+$') { return $Key }
    '(unnamed key)'
}

# An object's members as a dictionary whose keys compare by ordinal, so a key whose case changed differs.
function Get-Members($Object) {
    $members = New-Object 'System.Collections.Generic.Dictionary[string,object]' ([StringComparer]::Ordinal)
    foreach ($p in @($Object.PSObject.Properties)) { $members[$p.Name] = $p.Value }
    ,$members
}

# Adds to $Diffs the path of every key whose value differs between two parsed JSON values: objects key by key (a key
# on one side only differs, and keys compare case-sensitively), arrays element by element (an element on one side
# only differs, named <path>[<i>]), and leaves by type and text. Values are never kept, only key paths.
function Compare-Node($A, $B, [string]$Path, [System.Collections.ArrayList]$Diffs) {
    $objA = $A -is [System.Management.Automation.PSCustomObject]
    $objB = $B -is [System.Management.Automation.PSCustomObject]
    $arrA = $A -is [array]
    $arrB = $B -is [array]
    if ($objA -and $objB) {
        $ma = Get-Members $A
        $mb = Get-Members $B
        $names = New-Object System.Collections.ArrayList
        foreach ($name in @($ma.Keys) + @($mb.Keys)) { if (-not $names.Contains($name)) { [void]$names.Add($name) } }
        foreach ($name in $names) {
            $key = if ($Path) { "$Path.$name" } else { $name }
            if (-not ($ma.ContainsKey($name) -and $mb.ContainsKey($name))) { [void]$Diffs.Add($key); continue }
            Compare-Node $ma[$name] $mb[$name] $key $Diffs
        }
        return
    }
    if ($arrA -and $arrB) {
        $count = [math]::Max($A.Count, $B.Count)
        for ($i = 0; $i -lt $count; $i++) {
            $key = "$Path[$i]"
            if ($i -ge $A.Count -or $i -ge $B.Count) { [void]$Diffs.Add($key); continue }
            Compare-Node $A[$i] $B[$i] $key $Diffs
        }
        return
    }
    if ($objA -or $objB -or $arrA -or $arrB) { [void]$Diffs.Add($Path); return }
    if ($null -eq $A -and $null -eq $B) { return }
    if ($null -eq $A -or $null -eq $B) { [void]$Diffs.Add($Path); return }
    if ($A.GetType() -ne $B.GetType() -or [string]$A -cne [string]$B) { [void]$Diffs.Add($Path) }
}

# Puts one new level folder's measured files into the evidence copy's folder of the same name: the series files,
# streamhash.txt, graph.txt and streams.json (the rule reads its stream list), as the new pass wrote them, and
# manifest.txt as the new pass wrote it but with phase 0's run time, the one field T6-R13 leaves out and which golden
# check prints ("measured in <n> s"). The copy keeps phase 0's run-dependent files: audio-proxy.json (its times and
# ffmpeg line), command.txt (absolute paths), ffmpeg.txt, ffprobe.txt and proxy-check.json. A measured file that
# phase 0's folder has and the new one lacks is removed from the copy, so the difference shows.
function Set-NewLevelFiles([string]$From, [string]$Old, [string]$To) {
    $fixed = @('streamhash.txt', 'graph.txt', 'streams.json')
    $measured = @(@(Get-ChildItem -LiteralPath $From -File) + @(Get-ChildItem -LiteralPath $To -File) |
        ForEach-Object { $_.Name } |
        Where-Object { $_ -cmatch '^(main-mix|main-fc|cand-\d+)\.txt$' -or $fixed -ccontains $_ } | Sort-Object -Unique)
    foreach ($name in $measured) {
        if (Test-Path -LiteralPath "$From\$name" -PathType Leaf) {
            Copy-Item -LiteralPath "$From\$name" -Destination "$To\$name" -Force -ErrorAction Stop
        } else {
            Remove-Item -LiteralPath "$To\$name" -Force -ErrorAction Stop
        }
    }
    $time = Get-RunTime "$Old\manifest.txt"
    if (-not $time) { Stop-Part 'a manifest.txt of phase 0 has no run time on its second line' }
    $text = Set-RunTime (Read-Bytes "$From\manifest.txt") $time
    [IO.File]::WriteAllBytes("$To\manifest.txt", $Latin1.GetBytes($text))
}

# The evidence copy's audio-dataset.json as it was before a retune run: put back, or removed when there was none.
function Restore-Baseline([bool]$Had, [string]$Kept, [string]$Baseline) {
    if ($Had) { Copy-Item -LiteralPath $Kept -Destination $Baseline -Force -ErrorAction Stop }
    elseif (Test-Path -LiteralPath $Baseline) { Remove-Item -LiteralPath $Baseline -Force -ErrorAction Stop }
}

# A row's evidence.mks, found as engine-ocr-trial.ps1's inventory step finds it (lazymkv-dev ocr-trial inventory):
# the dataset copy beside the Tesseract reading, then the path the recording's subs-ocr\summary.json names, then
# phase 0's staging layout. $null when there is none.
function Find-Evidence($Track) {
    $candidates = @("$TuningData\$($Track.slug)\$($Track.label)\subs-ocr\evidence.mks")
    $summary = "$Recordings\$($Track.slug)\$($Track.label)\subs-ocr\summary.json"
    if (Test-Path -LiteralPath $summary -PathType Leaf) {
        $named = [string](Read-Json $summary).evidence
        if ($named) { $candidates += $named }
    }
    $candidates += "$Staging\$($Track.slug)\$($Track.label)\evidence.mks"
    foreach ($c in $candidates) { if (Test-Path -LiteralPath $c -PathType Leaf) { return $c } }
    $null
}

$made = $false
$savedPath = $env:Path
$savedTemp = @{ TEMP = $env:TEMP; TMP = $env:TMP; TMPDIR = $env:TMPDIR }
$go = $false
$haveFilm = $false
$levelsDone = $false
$listingDone = $false
$copyDone = $false
$pushed = $false
$hashesPrinted = $false
try {
    # The inputs and the candidate (call 12 (a): the two SHA-256 values must be equal before anything runs).
    try {
        if ($Self -and ($FfmpegSha256 -or $FfprobeSha256)) {
            Stop-Part 'give -Self or the two SHA-256 values, not both'
        }
        if (-not $Self) {
            foreach ($h in @($FfmpegSha256, $FfprobeSha256)) {
                if ($h -notmatch '^[0-9a-fA-F]{64}$') {
                    Stop-Part 'give -Self, or -FfmpegSha256 and -FfprobeSha256 as 64 hexadecimal digits each'
                }
            }
        }
        if (Test-Path -LiteralPath $Work) { Stop-Part 'the -Work folder exists already; remove it or name another' }
        . C:\lazymkv\src\scripts\phase0\env.ps1
        $fullFfmpeg = $FFMPEG
        if (-not $fullFfmpeg) { Stop-Part 'the installed ffmpeg was not found' }
        foreach ($exe in @($CAP, $DEV, $LevelsScript)) {
            if (-not (Test-Path -LiteralPath $exe -PathType Leaf)) {
                Stop-Part "$([IO.Path]::GetFileName($exe)) is missing"
            }
        }
        if ($Self) {
            'hashes: not checked (the installed build)'
            $hashesPrinted = $true
            if (-not $FFPROBE) { Stop-Part 'the installed ffprobe was not found' }
            $go = $true
        } else {
            $candFull = [IO.Path]::GetFullPath($Candidate).TrimEnd('\')
            foreach ($exe in @('ffmpeg.exe', 'ffprobe.exe')) {
                if (-not (Test-Path -LiteralPath "$candFull\$exe" -PathType Leaf)) {
                    Stop-Part "the candidate folder holds no $exe"
                }
            }
            $h1 = (Get-FileHash -LiteralPath "$candFull\ffmpeg.exe" -Algorithm SHA256).Hash
            $h2 = (Get-FileHash -LiteralPath "$candFull\ffprobe.exe" -Algorithm SHA256).Hash
            $same = ($h1 -ieq $FfmpegSha256) -and ($h2 -ieq $FfprobeSha256)
            "hashes: $(Get-Same $same)"
            $hashesPrinted = $true
            if (-not $same) {
                Add-Failure 'the candidate programs do not have the SHA-256 values given, so nothing else ran'
            } else {
                $env:Path = "$candFull;$savedPath"
                . C:\lazymkv\src\scripts\phase0\env.ps1
                if (-not $FFMPEG -or -not $FFPROBE) {
                    Stop-Part 'ffmpeg or ffprobe is not found with the candidate first on PATH'
                }
                $inFfmpeg = [IO.Path]::GetDirectoryName($FFMPEG).TrimEnd('\') -ieq $candFull
                $inFfprobe = [IO.Path]::GetDirectoryName($FFPROBE).TrimEnd('\') -ieq $candFull
                if (-not ($inFfmpeg -and $inFfprobe)) {
                    Stop-Part 'with the candidate first on PATH, env.ps1 still resolves ffmpeg or ffprobe outside it'
                }
                $go = $true
            }
        }
        New-Item -ItemType Directory -Path $Work -ErrorAction Stop | Out-Null
        $made = $true
    } catch {
        $go = $false
        if (-not $hashesPrinted) { 'hashes: not run' }
        Add-Failure "the inputs: $(Get-Reason $_)"
    }

    # The film: the one recording ending -uhd whose all-tracks folder holds a finished level folder, and its finished
    # level folders in the order phase 0's passes wrote them (T6-E15).
    if ($go) {
        try {
            $found = @()
            foreach ($r in @(Get-ChildItem -LiteralPath $Recordings -Directory -ErrorAction Stop)) {
                if ($r.Name -notlike '*-uhd') { continue }
                $at = "$($r.FullName)\$Label"
                if (-not (Test-Path -LiteralPath $at -PathType Container)) { continue }
                $levelDirs = @(Get-ChildItem -LiteralPath $at -Directory -Filter 'audio-levels-20ms-*')
                $finished = @()
                foreach ($l in $levelDirs) {
                    if ($l.Name -cmatch '^audio-levels-20ms-\d+(-\d+)*$' -and
                        (Test-Path -LiteralPath "$($l.FullName)\audio-proxy.json" -PathType Leaf)) {
                        $finished += $l.Name
                    }
                }
                if ($finished.Count -gt 0) { $found += [pscustomobject]@{ folder = $r.Name; levels = $finished } }
            }
            if ($found.Count -ne 1) {
                Stop-Part "$($found.Count) recordings ending -uhd hold a finished all-tracks level folder, not 1"
            }
            $film = $found[0].folder
            $levels = @($found[0].levels)
            $passes = @()
            foreach ($name in $levels) {
                $streams = @($name.Substring('audio-levels-20ms-'.Length) -split '-' | ForEach-Object { [int]$_ })
                [int[]]$others = @()
                if ($streams.Count -gt 1) { [int[]]$others = $streams[1..($streams.Count - 1)] }
                $folder = "$Recordings\$film\$Label\$name"
                $passes += [pscustomobject]@{
                    level = $name; main = $streams[0]; cands = $others
                    suffix = $name.Substring('audio-levels-20ms-'.Length)
                    first = Test-Path -LiteralPath "$folder\proxy-check.json" -PathType Leaf
                    time = (Get-Item -LiteralPath "$folder\manifest.txt" -ErrorAction Stop).LastWriteTimeUtc
                }
            }
            if (@($passes | ForEach-Object { $_.main } | Sort-Object -Unique).Count -ne 1) {
                Stop-Part "the $($levels.Count) finished level folders do not share one main stream"
            }
            $firsts = @($passes | Where-Object { $_.first }).Count
            if ($firsts -ne 1) {
                Stop-Part "$firsts of the $($levels.Count) finished level folders hold proxy-check.json, not 1"
            }
            $passes = @(@($passes | Where-Object { $_.first }) +
                @($passes | Where-Object { -not $_.first } | Sort-Object -Property time, level))
            $mkvs = @(Get-ChildItem -LiteralPath "$Staging\$film\$Label" -File -ErrorAction Stop |
                Where-Object { $_.Extension -eq '.mkv' })
            if ($mkvs.Count -ne 1) { Stop-Part "the film's staging folder holds $($mkvs.Count) .mkv files, not 1" }
            $rip = $mkvs[0].FullName
            $haveFilm = $true
        } catch {
            Add-Failure "finding the film: $(Get-Reason $_)"
        }
    }

    # levels: phase 0's passes repeated, under -Work only (T6-R12), each against phase 0's level folder of the same
    # name. Their output names the rip and paths, so none of it reaches the log; they run with the Continue preference
    # they were written for. A pass that does not finish stops the rest.
    if ($haveFilm) {
        try {
            foreach ($pass in $passes) {
                $main = $pass.main
                $cands = $pass.cands
                $ErrorActionPreference = 'Continue'
                try {
                    $levelOutput = @(& $LevelsScript -Slug $film -Label $Label -Main $main -Cand $cands `
                        -RecordingsRoot "$Work\recordings" -TuningRoot "$Work\dataset" *>&1)
                } finally { $ErrorActionPreference = 'Stop' }
                if ($levelOutput.Count -eq 0 -or [string]$levelOutput[-1] -cne 'RESULT ok') {
                    Stop-Part "the level pass $($pass.suffix) did not end RESULT ok"
                }
            }
            $levelsDone = $true
            $names = New-Object System.Collections.ArrayList
            $equal = 0
            $hash = $true
            $graph = $true
            $manifest = $true
            foreach ($pass in $passes) {
                $oldLevel = "$Recordings\$film\$Label\$($pass.level)"
                $newLevel = "$Work\recordings\$film\$Label\$($pass.level)"
                $series = @(@(Get-ChildItem -LiteralPath $oldLevel -File -ErrorAction Stop) +
                    @(Get-ChildItem -LiteralPath $newLevel -File -ErrorAction Stop) | ForEach-Object { $_.Name } |
                    Where-Object { $_ -cmatch '^(main-mix|main-fc|cand-\d+)\.txt$' } | Sort-Object -Unique)
                foreach ($name in $series) {
                    [void]$names.Add("$($pass.suffix) $name")
                    if (Test-SameBytes "$oldLevel\$name" "$newLevel\$name") { $equal++ }
                }
                if (-not (Test-SameBytes "$oldLevel\streamhash.txt" "$newLevel\streamhash.txt")) { $hash = $false }
                if (-not (Test-SameBytes "$oldLevel\graph.txt" "$newLevel\graph.txt")) { $graph = $false }
                if (-not (Test-SameManifest "$oldLevel\manifest.txt" "$newLevel\manifest.txt")) { $manifest = $false }
            }
            $different = $names.Count - $equal
            $passText = ($passes | ForEach-Object { $_.suffix }) -join ' then '
            "levels: $($names.Count) series files, $equal equal, $different different; streamhash $(Get-Same $hash); " +
                "graph $(Get-Same $graph); manifest $(Get-Same $manifest); passes $passText"
            if ($names.Count -eq 0 -or $different -gt 0 -or -not ($hash -and $graph -and $manifest)) {
                Add-Failure 'the level files differ from phase 0''s'
            }
        } catch {
            'levels: not run'
            Add-Failure "levels: $(Get-Reason $_)"
        }
    } else { 'levels: not run' }

    # proxy: the installed full ffmpeg's packet hashes of the new proxy and of phase 0's.
    if ($levelsDone) {
        try {
            $oldProxy = "$TuningData\$film\$Label\audio-proxy.mka"
            $newProxy = "$Work\dataset\$film\$Label\audio-proxy.mka"
            if (-not (Test-Path -LiteralPath $oldProxy -PathType Leaf)) { Stop-Part 'phase 0''s proxy is missing' }
            if (-not (Test-Path -LiteralPath $newProxy -PathType Leaf)) { Stop-Part 'the level pass wrote no proxy' }
            $hashArgs = @('-v', 'error', '-i', '', '-map', '0', '-c', 'copy', '-f', 'streamhash', '-hash', 'md5', '-')
            $hashArgs[3] = $oldProxy
            $code = Invoke-Recorded "$Work\proxy\old.streamhash.txt" $fullFfmpeg $hashArgs
            if ($code -ne 0) { Stop-Part "the streamhash of phase 0's proxy exited $code" }
            $hashArgs[3] = $newProxy
            $code = Invoke-Recorded "$Work\proxy\new.streamhash.txt" $fullFfmpeg $hashArgs
            if ($code -ne 0) { Stop-Part "the streamhash of the new proxy exited $code" }
            $tracks = @([IO.File]::ReadAllLines("$Work\proxy\new.streamhash.txt") |
                Where-Object { $_ -match '^\d+,a,MD5=' }).Count
            $same = Test-SameBytes "$Work\proxy\old.streamhash.txt" "$Work\proxy\new.streamhash.txt"
            "proxy: $tracks tracks, packet hashes $(Get-Same $same)"
            if (-not $same -or $tracks -eq 0) { Add-Failure 'the proxy''s packet hashes differ from phase 0''s' }
        } catch {
            'proxy: not run'
            Add-Failure "proxy: $(Get-Reason $_)"
        }
    } else { 'proxy: not run' }

    # listing: the candidate ffprobe with probe_args' words over the kept rip, against ffprobe-streams.json.
    if ($haveFilm) {
        try {
            $listing = "$Work\listing\ffprobe-streams.json"
            $probeArgs = @('-v', 'error', '-show_streams', '-show_format', '-of', 'json', $rip)
            $code = Invoke-Recorded $listing $FFPROBE $probeArgs
            if ($code -ne 0) { Stop-Part "the candidate ffprobe's listing of the kept rip exited $code" }
            $listingDone = $true
            $oldListing = Read-Json "$Recordings\$film\$Label\ffprobe-streams.json"
            $newListing = Read-Json $listing
            $oldStreams = @($oldListing.streams)
            $newStreams = @($newListing.streams)
            $audio = 0
            $audioKeys = 0
            $otherKeys = 0
            $otherNames = New-Object System.Collections.ArrayList
            $count = [math]::Max($oldStreams.Count, $newStreams.Count)
            for ($i = 0; $i -lt $count; $i++) {
                $o = if ($i -lt $oldStreams.Count) { $oldStreams[$i] } else { $null }
                $n = if ($i -lt $newStreams.Count) { $newStreams[$i] } else { $null }
                $kind = if ($null -ne $o) { [string]$o.codec_type } else { [string]$n.codec_type }
                if ($kind -cnotmatch '^[a-z]+$') { $kind = 'unknown' }
                $diffs = New-Object System.Collections.ArrayList
                if ($null -eq $o -or $null -eq $n) { [void]$diffs.Add('(the whole stream)') }
                else { Compare-Node $o $n '' $diffs }
                if ($kind -ceq 'audio') {
                    if ($null -ne $o) { $audio++ }
                    $audioKeys += $diffs.Count
                } else {
                    $otherKeys += $diffs.Count
                    foreach ($d in $diffs) {
                        $pair = "$kind $(Get-KeyName $d)"
                        if (-not $otherNames.Contains($pair)) { [void]$otherNames.Add($pair) }
                    }
                }
            }
            # The format section is reported as the kind format, as other streams are, and fails nothing.
            $diffs = New-Object System.Collections.ArrayList
            Compare-Node $oldListing.format $newListing.format '' $diffs
            $otherKeys += $diffs.Count
            foreach ($d in $diffs) {
                $pair = if ($d) { "format $(Get-KeyName $d)" } else { 'format (the whole section)' }
                if (-not $otherNames.Contains($pair)) { [void]$otherNames.Add($pair) }
            }
            $named = if ($otherNames.Count -gt 0) { ': ' + ($otherNames -join ', ') } else { '' }
            "listing: $audio audio streams, $audioKeys differing keys; other streams: $otherKeys differing keys$named"
            if ($audioKeys -gt 0) { Add-Failure 'a key of an audio stream differs in the listing' }
            if ($audio -eq 0) { Add-Failure 'phase 0''s listing holds no audio stream' }
        } catch {
            'listing: not run'
            Add-Failure "listing: $(Get-Reason $_)"
        }
    } else { 'listing: not run' }

    # packets row <r>: packets_args' words over each row's evidence.mks, against the recording's packet listing.
    $rows = $null
    if ($go) {
        try { $rows = @(Get-TrialRows) } catch { Add-Failure "the trial's rows: $(Get-Reason $_)" }
    }
    for ($r = 1; $r -le 6; $r++) {
        if ($null -eq $rows) { "packets row ${r}: not run"; continue }
        try {
            $t = $rows[$r - 1]
            $mks = Find-Evidence $t
            if (-not $mks) { Stop-Part "row ${r}: no evidence.mks was found" }
            $packets = "$Work\packets\row-$r.json"
            $packetArgs = @('-v', 'error', '-select_streams', "s:$($t.stream)", '-show_entries',
                'packet=pts_time,duration_time,size', '-of', 'json', $mks)
            $code = Invoke-Recorded $packets $FFPROBE $packetArgs
            if ($code -ne 0) { Stop-Part "row ${r}: the packet listing exited $code" }
            $same = Test-SameBytes $packets "$Recordings\$($t.slug)\$($t.label)\subs\sub-$($t.stream).packets.json"
            "packets row ${r}: $(Get-Same $same)"
            if (-not $same) { Add-Failure "the packet listing of row $r differs" }
        } catch {
            "packets row ${r}: not run"
            Add-Failure "packets row ${r}: $(Get-Reason $_)"
        }
    }

    # corpus copy: the whole evidence mirror into <Work>\evidence, refused above 4 GB.
    if ($go) {
        try {
            if (-not (Test-Path -LiteralPath $Evidence -PathType Container)) {
                Stop-Part 'the evidence mirror is missing'
            }
            $files = @(Get-ChildItem -LiteralPath $Evidence -Recurse -Force -File -ErrorAction Stop)
            $bytes = [long]($files | Measure-Object -Property Length -Sum).Sum
            if ($bytes -gt $MaxCopyBytes) {
                "corpus copy: $($files.Count) files, $bytes bytes, refused (above 4 GB)"
                Add-Failure 'the evidence mirror is above 4 GB, so it was not copied'
            } else {
                Copy-Item -LiteralPath $Evidence -Destination "$Work\evidence" -Recurse -Force -ErrorAction Stop
                $copied = @(Get-ChildItem -LiteralPath "$Work\evidence" -Recurse -Force -File -ErrorAction Stop)
                $copiedBytes = [long]($copied | Measure-Object -Property Length -Sum).Sum
                if ($copied.Count -ne $files.Count -or $copiedBytes -ne $bytes) {
                    Stop-Part ("the copy holds $($copied.Count) files and $copiedBytes bytes, " +
                        "not $($files.Count) and $bytes")
                }
                "corpus copy: $($files.Count) files, $bytes bytes"
                $copyDone = $true
            }
        } catch {
            'corpus copy: not run'
            Add-Failure "corpus copy: $(Get-Reason $_)"
        }
    } else { 'corpus copy: not run' }

    # golden check and retune over the copy, before and after the film's new listing and level folder go into it.
    $exits = @{}
    if ($copyDone) {
        try {
            New-Item -ItemType Directory -Path "$Work\temp" -ErrorAction Stop | Out-Null
            foreach ($name in @('TEMP', 'TMP', 'TMPDIR')) { Set-Item -Path "Env:$name" -Value "$Work\temp" }
            # lazymkv-dev finds the spec (row 13, which labels.json and the retune golden read) from the repository
            # root above its current folder, so both commands run in the synced source tree.
            if (-not (Test-Path -LiteralPath "$RepoRoot\Cargo.toml" -PathType Leaf)) {
                Stop-Part 'the source tree is missing'
            }
            Push-Location -LiteralPath $RepoRoot
            $pushed = $true
            $goldenArgs = @('golden', 'check', '--evidence', "$Work\evidence")
            $retuneArgs = @('retune', '--evidence', "$Work\evidence", '--spec', $Spec)
            # retune writes its snapshot over the copy's audio-dataset.json when it differs; that file is put back
            # after each retune run, so both golden check runs read the mirror's own baseline.
            New-Item -ItemType Directory -Force -Path "$Work\runs" -ErrorAction Stop | Out-Null
            $baseline = "$Work\evidence\audio-dataset.json"
            $keptBaseline = "$Work\runs\audio-dataset.kept"
            $hadBaseline = Test-Path -LiteralPath $baseline -PathType Leaf
            if ($hadBaseline) { Copy-Item -LiteralPath $baseline -Destination $keptBaseline -ErrorAction Stop }
            $exits['golden-before'] = Invoke-Recorded "$Work\runs\golden-before.txt" $DEV $goldenArgs
            $exits['retune-before'] = Invoke-Recorded "$Work\runs\retune-before.txt" $DEV $retuneArgs
            Restore-Baseline $hadBaseline $keptBaseline $baseline
            if ($levelsDone -and $listingDone) {
                $dest = "$Work\evidence\$film\$Label"
                if (-not (Test-Path -LiteralPath "$dest\ffprobe-streams.json" -PathType Leaf)) {
                    Stop-Part 'the evidence copy holds no listing of the film'
                }
                Copy-Item -LiteralPath $listing -Destination "$dest\ffprobe-streams.json" -Force -ErrorAction Stop
                foreach ($pass in $passes) {
                    $to = "$dest\$($pass.level)"
                    $from = "$Work\recordings\$film\$Label\$($pass.level)"
                    $old = "$Recordings\$film\$Label\$($pass.level)"
                    if (-not (Test-Path -LiteralPath $to -PathType Container)) {
                        Stop-Part "the evidence copy holds no level folder $($pass.suffix) of the film"
                    }
                    Set-NewLevelFiles $from $old $to
                }
                $exits['golden-after'] = Invoke-Recorded "$Work\runs\golden-after.txt" $DEV $goldenArgs
                $exits['retune-after'] = Invoke-Recorded "$Work\runs\retune-after.txt" $DEV $retuneArgs
                Restore-Baseline $hadBaseline $keptBaseline $baseline
            }
        } catch {
            Add-Failure "golden check and retune: $(Get-Reason $_)"
        } finally {
            if ($pushed) { Pop-Location }
            foreach ($name in @('TEMP', 'TMP', 'TMPDIR')) {
                if ($savedTemp[$name]) { Set-Item -Path "Env:$name" -Value $savedTemp[$name] }
                elseif (Test-Path "Env:$name") { Remove-Item -Path "Env:$name" }
            }
        }
    }
    $parts = @(@{ key = 'golden'; line = 'golden check'; exits = @(0) },
        @{ key = 'retune'; line = 'retune'; exits = @(0, 1, 3) })
    foreach ($part in $parts) {
        $before = "$($part.key)-before"
        $after = "$($part.key)-after"
        if (-not ($exits.ContainsKey($before) -and $exits.ContainsKey($after))) {
            "$($part.line): not run"
            if ($copyDone) { Add-Failure "$($part.line) did not run before and after" }
            continue
        }
        $one = "$Work\runs\$before.txt"
        $two = "$Work\runs\$after.txt"
        $sameOut = Test-SameBytes $one $two
        $sameErr = Test-SameBytes "$one.stderr" "$two.stderr"
        $word = if ($sameOut -and $sameErr) { 'same output' } else { 'different output' }
        "$($part.line): $word; exit $($exits[$before]) then $($exits[$after])"
        if (-not $sameOut) {
            Add-Failure ("$($part.line)'s output changed with the film's new listing and level folders (" +
                "$(Get-LineDiff $one $two))")
        }
        if (-not $sameErr) {
            Add-Failure ("$($part.line)'s standard error changed with the film's new listing and level folders (" +
                "$(Get-LineDiff "$one.stderr" "$two.stderr"))")
        }
        if ($exits[$before] -ne $exits[$after]) { Add-Failure "$($part.line)'s exit code changed" }
        # The run before read the copy: a known exit and some standard output (the header's rule).
        $code = $exits[$before]
        if (@($part.exits) -notcontains $code) { Add-Failure "$($part.line) refused the evidence copy (exit $code)" }
        $printed = (Test-Path -LiteralPath $one -PathType Leaf) -and ((Get-Item -LiteralPath $one).Length -gt 0)
        if (-not $printed) { Add-Failure "$($part.line) printed nothing on its standard output (exit $code)" }
    }
} catch {
    Add-Failure "the check stopped: $(Get-Reason $_)"
} finally {
    $env:Path = $savedPath
    if ($made) {
        try { Remove-Item -LiteralPath $Work -Recurse -Force -ErrorAction Stop } catch { }
        if (Test-Path -LiteralPath $Work) { Add-Failure 'the -Work folder could not be removed' }
    }
}
if ($failures.Count -eq 0) {
    'RESULT ok'
} else {
    "RESULT FAILED: $($failures -join '; ')"
    throw 'ffmpeg-check.ps1 failed (the RESULT line says why)'
}
