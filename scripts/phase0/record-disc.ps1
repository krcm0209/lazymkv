# Records what can be learned about the inserted disc without ripping it.
param(
    [Parameter(Mandatory)][string]$Slug,
    [switch]$NoScan,
    [string]$ScanName = 'scan'
)
. C:\lazymkv\src\scripts\phase0\env.ps1
$out = "$P0\recordings\$Slug"
New-Item -ItemType Directory -Force $out | Out-Null

$label = (Get-Volume -DriveLetter $Drive).FileSystemLabel
if (-not $label) { $label = '(none)' }
& $CAP disc --root "${Drive}:\" --label "$label" --out $out | Out-Null
$meta = Get-Content "$out\meta.json" -Raw | ConvertFrom-Json
"disc: label=$label format=$($meta.format) fingerprint=$($meta.fingerprint) disc_id=$($meta.disc_id)"

$status = @()
if ($meta.fingerprint) {
    $code = curl.exe -s -m 30 -o "$out\thediscdb-hash.json" -w '%{http_code}' "https://thediscdb.com/api/dischash/$($meta.fingerprint)"
    $status += "dischash $code"
}
if ($meta.disc_id) {
    $code = curl.exe -s -m 30 -o "$out\thediscdb-discid.json" -w '%{http_code}' "https://thediscdb.com/api/discid/$($meta.disc_id)"
    $status += "discid $code"
}

$tmdbId = $null
$kind = $null
try {
    $hit = Get-Content "$out\thediscdb-hash.json" -Raw -ErrorAction Stop | ConvertFrom-Json
    $tmdbId = $hit.media.external_ids.tmdb
    $kind = if ($hit.media.type -eq 'Movie') { 'movie' } else { 'tv' }
} catch { }
if ($tmdbId -and $env:TMDB_TOKEN) {
    $code = curl.exe -s -m 30 -o "$out\tmdb.json" -w '%{http_code}' -H "Authorization: Bearer $env:TMDB_TOKEN" "https://api.themoviedb.org/3/$kind/${tmdbId}?append_to_response=external_ids"
    $status += "tmdb $kind/$tmdbId $code"
}
Set-Content "$out\lookup-status.txt" $status
"lookups: $($status -join '; ')"

if (-not $NoScan) {
    $index = Get-DiscIndex
    $flags = Get-MkFlags
    & $CAP record --out "$out\$ScanName.txt" -- $MK @flags info "disc:$index" | Out-Null
    "scan: " + (Get-Content "$out\$ScanName.txt.exit.json")
}
