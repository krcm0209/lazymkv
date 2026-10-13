# Records what can be learned about the disc in $Drive without ripping it, in $Recordings\<Slug>\
# (C:\lazymkv\phase0\recordings\<Slug>\):
#   meta.json, files.json, nav\     from lazymkv-capture disc (navigation-file copies; never Unit_Key_RO.inf)
#   thediscdb-hash.json, thediscdb-discid.json, tmdb.json, lookup-status.txt     the lookups (tmdb.json slimmed to
#                                   original_language, the ids, title and date, and fetched_at: TMDb's 6-month cap)
#   <ScanName>.txt with .timing.jsonl, .stderr and .exit.json     MakeMKV's scan (unless -NoScan)
# A new slug gets the snapshot and the lookups. An existing recording is only ever extended with the
# same disc (Assert-DiscIs); its snapshot and lookups are kept, or replaced by new ones with -Force.
# An existing scan file is refused unless -Force is given. -ScanMayFail is for a scan whose failure is
# itself the result (Task 9's offline scan): the scan then only has to prove that MakeMKV ran.
# The last line is "RESULT ok" or "RESULT FAILED: <reason>"; after a failure the script throws.
param(
    [Parameter(Mandatory)][string]$Slug,
    [switch]$NoScan,
    [string]$ScanName = 'scan',
    [switch]$ScanMayFail,
    [switch]$Force
)
. C:\lazymkv\src\scripts\phase0\env.ps1

try {
    Assert-Slug $Slug
    Assert-Slug $ScanName
    $note = ''
    $out = "$Recordings\$Slug"
    $stage = "$P0\tmp\record-$Slug"
    $scan = "$out\$ScanName.txt"
    $scanFiles = @($scan, "$scan.stderr", "$scan.timing.jsonl", "$scan.exit.json", "$scan.makemkv-debug.txt")

    # Refuse before anything is created, deleted or started.
    if (-not (Test-Path -LiteralPath $CAP)) { throw "missing $CAP; build it with: scripts/box.sh cargo build --release -p lazymkv-capture" }
    if (-not $NoScan) {
        if (-not (Test-Path -LiteralPath $AllTracksProfile)) { throw "missing job profile $AllTracksProfile" }
        foreach ($file in $scanFiles) {
            if ((Test-Path -LiteralPath $file) -and -not $Force) {
                throw "$file already exists and is kept as evidence; choose another -ScanName, or use -Force only where the plan says so"
            }
        }
    }
    $recorded = Test-Path -LiteralPath "$out\meta.json"
    if (-not $recorded -and @(Get-ChildItem -LiteralPath $out -Filter '*.exit.json' -ErrorAction SilentlyContinue).Count -gt 0) {
        throw "$out holds recorded runs but no meta.json (an earlier -Force run stopped part-way); put its meta.json back before running again"
    }

    # The disc may still be spinning up: wait up to two minutes until its root can be listed.
    $ready = $false
    for ($i = 0; $i -lt 24 -and -not $ready; $i++) {
        if (@(Get-ChildItem -LiteralPath "${Drive}:\" -ErrorAction SilentlyContinue).Count -gt 0) { $ready = $true } else { Start-Sleep -Seconds 5 }
    }
    if (-not $ready) { throw "no readable disc in ${Drive}: after 120 s" }

    # An existing recording is only ever extended with the same disc.
    if ($recorded) { Assert-DiscIs $Slug }

    if (-not $recorded -or $Force) {
        # Snapshot and lookups go to an empty local folder first, and replace the recording's files only when complete.
        if (Test-Path -LiteralPath $stage) { Remove-Item -LiteralPath $stage -Recurse -Force -ErrorAction Stop }
        Assert-EmptyDir $stage
        $label = $null
        try { $label = ([System.IO.DriveInfo]::new($Drive)).VolumeLabel } catch { }
        if (-not $label) { $label = '(none)' }
        & $CAP disc --root "${Drive}:\" --label $label --out $stage | Out-Null
        $capExit = $LASTEXITCODE
        if ($capExit -ne 0 -or -not (Test-Path -LiteralPath "$stage\meta.json")) { throw "lazymkv-capture disc failed (exit $capExit)" }
        $meta = Get-Content -LiteralPath "$stage\meta.json" -Raw -ErrorAction Stop | ConvertFrom-Json
        if (-not $meta.fingerprint) { throw "the disc has no fingerprint (no BDMV\STREAM or VIDEO_TS files were listed); see $stage\files.json" }
        "disc: label=$label format=$($meta.format) fingerprint=$($meta.fingerprint) disc_id=$($meta.disc_id) files=$($meta.file_count)"

        $status = @()
        $hashCode = curl.exe -s -m 30 -o "$stage\thediscdb-hash.json" -w '%{http_code}' "https://thediscdb.com/api/dischash/$($meta.fingerprint)"
        $status += "dischash $hashCode"
        if ($meta.disc_id) {
            $idCode = curl.exe -s -m 30 -o "$stage\thediscdb-discid.json" -w '%{http_code}' "https://thediscdb.com/api/discid/$($meta.disc_id)"
            $status += "discid $idCode"
        }
        $tmdbNote = 'tmdb not tried: TheDiscDB gave no TMDb ID'
        $tmdbId = $null
        if ($hashCode -eq '200') {
            try {
                $hit = Get-Content -LiteralPath "$stage\thediscdb-hash.json" -Raw -ErrorAction Stop | ConvertFrom-Json
                $tmdbId = [string]$hit.media.external_ids.tmdb
                # The ID comes from community data and goes onto a command line next to the token: digits only.
                if ($tmdbId -notmatch '^\d+$') { $tmdbId = $null }
                $kind = if ($hit.media.type -eq 'Movie') { 'movie' } else { 'tv' }
            } catch { }
        }
        if ($tmdbId -and -not $env:TMDB_TOKEN) { $tmdbNote = 'tmdb not tried: TMDB_TOKEN is not set in this session' }
        if ($tmdbId -and $env:TMDB_TOKEN) {
            $tmdbCode = curl.exe -s -m 30 -o "$stage\tmdb.json" -w '%{http_code}' -H "Authorization: Bearer $env:TMDB_TOKEN" "https://api.themoviedb.org/3/$kind/${tmdbId}?append_to_response=external_ids"
            $status += "tmdb $kind/$tmdbId $tmdbCode"
            if ($tmdbCode -eq '200') {
                # TMDb's terms let a copy of its data be kept six months at most: only what the scripts read
                # (original_language), the ids, the work's title and date and the day of the fetch stay, and the file
                # is fetched again once fetched_at is six months old (corpus/README.md).
                try {
                    $full = Get-Content -LiteralPath "$stage\tmdb.json" -Raw -Encoding UTF8 -ErrorAction Stop | ConvertFrom-Json
                    $slim = [ordered]@{}
                    foreach ($field in 'id', 'imdb_id', 'title', 'original_title', 'release_date', 'name', 'original_name', 'first_air_date', 'original_language') {
                        if ($null -ne $full.$field) { $slim[$field] = $full.$field }
                    }
                    $ids = [ordered]@{}
                    foreach ($field in 'imdb_id', 'tvdb_id', 'wikidata_id') {
                        if ($full.external_ids.$field) { $ids[$field] = $full.external_ids.$field }
                    }
                    if ($ids.Count -gt 0) { $slim['external_ids'] = $ids }
                    $slim['fetched_at'] = (Get-Date).ToString('yyyy-MM-dd')
                    Set-Content -LiteralPath "$stage\tmdb.json" -Value ($slim | ConvertTo-Json -Depth 3) -Encoding UTF8 -ErrorAction Stop
                } catch {
                    # A full answer is never kept: without its slim form there is no tmdb.json.
                    Remove-Item -LiteralPath "$stage\tmdb.json" -Force -ErrorAction Stop
                    $status += "tmdb answer not kept: $($_.Exception.Message)"
                }
            }
            $tmdbNote = $null
        }
        Set-Content -LiteralPath "$stage\lookup-status.txt" -Value $status -Encoding UTF8 -ErrorAction Stop
        "lookups: $($status -join '; ')" + $(if ($tmdbNote) { "; $tmdbNote" } else { '' })

        # Replace the recording's snapshot and lookups. meta.json goes first and comes back last,
        # so a recording that has a meta.json is always complete.
        New-Item -ItemType Directory -Force -Path $out -ErrorAction Stop | Out-Null
        foreach ($name in 'meta.json', 'files.json', 'nav', 'thediscdb-hash.json', 'thediscdb-discid.json', 'tmdb.json', 'lookup-status.txt') {
            if (Test-Path -LiteralPath "$out\$name") { Remove-Item -LiteralPath "$out\$name" -Recurse -Force -ErrorAction Stop }
        }
        foreach ($name in 'files.json', 'nav', 'thediscdb-hash.json', 'thediscdb-discid.json', 'tmdb.json', 'lookup-status.txt', 'meta.json') {
            if (Test-Path -LiteralPath "$stage\$name") { Move-Item -LiteralPath "$stage\$name" -Destination "$out\$name" -ErrorAction Stop }
        }
        Remove-Item -LiteralPath $stage -Recurse -Force -ErrorAction Stop
    } else {
        $meta = Get-Content -LiteralPath "$out\meta.json" -Raw -ErrorAction Stop | ConvertFrom-Json
        "disc: kept from the earlier recording of this same disc: label=$($meta.volume_label) format=$($meta.format) fingerprint=$($meta.fingerprint) disc_id=$($meta.disc_id) files=$($meta.file_count)"
        "lookups: kept: " + ((Get-Content -LiteralPath "$out\lookup-status.txt" -ErrorAction SilentlyContinue) -join '; ')
    }

    if (-not $NoScan) {
        # Scan files exist here only under -Force: remove them, so that stale files cannot pass for this scan's.
        foreach ($file in $scanFiles) {
            if (Test-Path -LiteralPath $file) { Remove-Item -LiteralPath $file -Force -ErrorAction Stop }
        }
        $index = Get-DiscIndex
        $flags = Get-MkFlags
        $scanStart = Get-Date
        & $CAP record --out $scan -- $MK @flags info "disc:$index" | Out-Null
        Save-MkDebugLog -Since $scanStart -To "$scan.makemkv-debug.txt"
        $exitJson = [string](Get-Content -LiteralPath "$scan.exit.json" -Raw -ErrorAction SilentlyContinue)
        $raw = [string](Get-Content -LiteralPath $scan -Raw -ErrorAction SilentlyContinue)
        $banner = [regex]::IsMatch($raw, '(?m)^MSG:1005,')
        $count = [regex]::Match($raw, '(?m)^TCOUNT:(\d+)')
        $titles = if ($count.Success) { $count.Groups[1].Value } else { 'NO-TCOUNT-LINE' }
        "scan: $ScanName.txt exit.json=$(if ($exitJson) { $exitJson.Trim() } else { 'MISSING' }) version_banner=$banner titles=$titles"
        if (-not $exitJson) { throw "the scan did not finish: $scan.exit.json was not written" }
        if (-not $banner) { throw "MakeMKV did not run: no MSG:1005 version banner in $scan" }
        $listed = $count.Success -and ([int]$titles -gt 0)
        if (-not $listed -and -not $ScanMayFail) { throw "the scan lists no titles (titles=$titles)" }
        if (-not $listed) { $note = ' (MakeMKV ran; the scan lists no titles, accepted because of -ScanMayFail)' }
    }
    "RESULT ok$note"
} catch {
    "RESULT FAILED: $($_.Exception.Message)"
    throw
}
