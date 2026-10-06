# Decision D16: copies one title's part of the tuning dataset ($TuningData\<Slug>\<Label>\: audio-proxy.mka,
# audio-proxy.json and the audio-levels-20ms-* folders; with -Text instead only the OCR text of its subtitle tracks,
# subs-ocr\sub-<n>.ocr.jsonl, written by ocr-subs.ps1, and ocrs-s<n>.jsonl, written by ocrs-trial.ps1) to C:\lazymkv\phase0\tmp\tuning-fetch\<Slug>\<Label>\, so that
# the Mac can fetch it with scp: the share is reachable only from a job. Needed only when a measure needs the proxies
# on the Mac; the routine after a rip never runs it.
# This is the one exception to "only small files on C:" (plan, Global Constraints): one title at a time, at most 2 GB,
# for the minutes an scp takes. The script prints the size and the two commands the Mac runs: the scp, then, at once,
# the clean-up that deletes the copy. The dataset itself is never changed.
# Refuses when the title has no complete proxy (no audio-proxy.json), when its folder holds more than 2 GB, or when an
# earlier fetch is still on C: (C:\lazymkv\phase0\tmp\tuning-fetch exists), unless -Force is given: it then deletes
# that earlier copy first (only C:\lazymkv\phase0\tmp\tuning-fetch, never anything in the dataset).
# The last line is "RESULT ok" or "RESULT FAILED: <reason>"; after a failure the script throws.
param(
    [Parameter(Mandatory)][string]$Slug,
    [Parameter(Mandatory)][string]$Label,
    [switch]$Text,
    [switch]$Force
)
. C:\lazymkv\src\scripts\phase0\env.ps1

$MaxBytes = 2GB
# How the Mac reaches the mini PC (the default of scripts/phase0/job.sh, wait.sh and pull-recording.sh).
$MacHost = 'USER@makemkv-box'

# "<path below the folder>=<bytes>" for every file of a folder, sorted: two folders hold the same files when these match.
function Get-Listing([string]$Root) {
    @(Get-ChildItem -LiteralPath $Root -Recurse -File -ErrorAction Stop |
        ForEach-Object { "$($_.FullName.Substring($Root.Length))=$($_.Length)" } | Sort-Object) -join ' '
}

$made = $null
try {
    Assert-Slug $Slug
    Assert-Slug $Label
    if (-not $TuningData) { throw "`$TuningData is not set (C:\lazymkv\phase0\config.ps1)" }
    if (-not (Test-Path -LiteralPath $TuningData)) { throw "cannot reach the tuning dataset $TuningData" }
    $src = "$TuningData\$Slug\$Label"
    if ($Text) {
        # Only the text files: the frames and tesseract's raw output beside them stay on the share.
        $src = "$src\subs-ocr"
        $files = @(Get-ChildItem -LiteralPath $src -Filter '*.ocr.jsonl' -File -ErrorAction SilentlyContinue)
        if ($files.Count -eq 0) { throw "$src holds no OCR text (sub-<n>.ocr.jsonl; run ocr-subs.ps1 first)" }
        # ocrs's text of the same frames (ocrs-trial.ps1), when there is any, for scripts/phase0/ocr_compare.py.
        $files += @(Get-ChildItem -LiteralPath $src -Filter 'ocrs-s*.jsonl' -File -ErrorAction SilentlyContinue)
    } else {
        if (-not (Test-Path -LiteralPath "$src\audio-proxy.json")) { throw "$src holds no complete proxy (no audio-proxy.json)" }
        $files = @(Get-ChildItem -LiteralPath $src -Recurse -File -ErrorAction Stop)
    }
    $bytes = [long]($files | Measure-Object -Property Length -Sum).Sum
    if ($bytes -gt $MaxBytes) {
        throw "$src holds $bytes bytes, more than the 2 GB a fetch may put on C:; nothing was copied (the owner copies it from the share another way)"
    }
    $root = "$P0\tmp\tuning-fetch"
    $dst = "$root\$Slug\$Label" + $(if ($Text) { '\subs-ocr' } else { '' })
    $cleanup = "ssh $MacHost `"rmdir /s /q $root`""
    if ((Test-Path -LiteralPath $root) -and -not $Force) {
        throw "$root already exists: an earlier fetch was not deleted after its scp. Delete it from the Mac with: $cleanup (or use -Force)"
    }
    if (Test-Path -LiteralPath $root) {
        Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction Stop
        "fetch: deleted $root, left by an earlier fetch"
    }
    $made = $root
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $dst) -ErrorAction Stop | Out-Null
    if ($Text) {
        New-Item -ItemType Directory -Force -Path $dst -ErrorAction Stop | Out-Null
        foreach ($file in $files) { Copy-Item -LiteralPath $file.FullName -Destination $dst -ErrorAction Stop }
        # Same form as Get-Listing (a backslash before each name).
        $here = @($files | ForEach-Object { "\$($_.Name)=$($_.Length)" } | Sort-Object) -join ' '
    } else {
        Copy-Item -LiteralPath $src -Destination $dst -Recurse -ErrorAction Stop
        $here = Get-Listing $src
    }
    $there = Get-Listing $dst
    if ($here -ne $there) { throw "the copy in $dst differs from $src" }
    $copied = [long](Get-ChildItem -LiteralPath $dst -Recurse -File -ErrorAction Stop | Measure-Object -Property Length -Sum).Sum
    "fetched: $src -> $dst ($copied bytes)"
    "scp (on the Mac; DEST is a folder there that does not exist yet): scp -q -r -o BatchMode=yes ${MacHost}:$($dst -replace '\\', '/') DEST"
    "clean-up (on the Mac, right after the scp): $cleanup"
    'RESULT ok'
} catch {
    $failure = $_
    # Only a copy this run started is removed; a refusal leaves an existing copy alone.
    if ($made -and (Test-Path -LiteralPath $made)) { Remove-Item -LiteralPath $made -Recurse -Force -ErrorAction SilentlyContinue }
    "RESULT FAILED: $($failure.Exception.Message)"
    throw $failure
}
