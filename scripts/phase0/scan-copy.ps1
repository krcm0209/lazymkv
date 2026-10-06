# Experiment 3 (Task 14, after backup-disc.ps1): scans the whole-disc copy in $Staging\<Slug>\backup\ again, on its
# own, so that a failed scan of the copy can be looked at without repeating the copy. Nothing is copied or deleted.
# Writes to $Recordings\<Slug>\backup\ (C:\lazymkv\phase0\recordings\<Slug>\backup\):
#   scan-<Name>.txt with .timing.jsonl, .stderr and .exit.json   MakeMKV's output, recorded by lazymkv-capture
#   scan-<Name>.txt.makemkv-debug.txt   MakeMKV's debug log of the run, when the flags include --debug (decision D16)
# -Flags chooses the flags: default = Get-MkFlags (the disc-time scan's flags, with --debug and the job profile);
# nodebug = the same without --debug; plain = -r --messages=-stdout --progress=-same only.
# -Target chooses what MakeMKV opens: root = the copy's root (the folder holding BDMV), bdmv = its BDMV folder.
# The scan succeeded when MakeMKV finished (exit.json written), printed the MSG:1005 banner and lists at least one
# title. MakeMKV's exit code is recorded and printed, never judged by itself (spec 9.2).
# An existing scan-<Name>.txt is refused unless -Force is given.
# The last line is "RESULT ok" or "RESULT FAILED: <reason>"; after a failure the script throws.
param(
    [Parameter(Mandatory)][string]$Slug,
    [Parameter(Mandatory)][string]$Name,
    [ValidateSet('default', 'nodebug', 'plain')][string]$Flags = 'default',
    [ValidateSet('root', 'bdmv')][string]$Target = 'root',
    [switch]$Force
)
. C:\lazymkv\src\scripts\phase0\env.ps1

try {
    Assert-Slug $Slug
    Assert-Slug $Name
    if (-not (Test-Path -LiteralPath $CAP)) { throw "missing $CAP; build it with: scripts/box.sh cargo build --release -p lazymkv-capture" }
    $rec = "$Recordings\$Slug\backup"
    $summaryFile = "$rec\backup-summary.json"
    if (-not (Test-Path -LiteralPath $summaryFile)) { throw "no $summaryFile; make the copy with backup-disc.ps1 first" }
    $backup = Get-Content -LiteralPath $summaryFile -Raw -ErrorAction Stop | ConvertFrom-Json
    $root = [string]$backup.copy_root
    if (-not $root) { throw "backup-summary.json names no copy_root: the copy did not succeed" }
    if (-not (Test-Path -LiteralPath $root)) { throw "the copy root $root cannot be reached" }
    $out = "$rec\scan-$Name.txt"
    if ((Test-Path -LiteralPath $out) -and -not $Force) { throw "$out already exists and is kept as evidence; choose another name or use -Force" }
    foreach ($suffix in '', '.timing.jsonl', '.stderr', '.exit.json', '.makemkv-debug.txt') {
        if (Test-Path -LiteralPath "$out$suffix") { Remove-Item -LiteralPath "$out$suffix" -Force -ErrorAction Stop }
    }
    $running = @(Get-Process -Name makemkvcon64, makemkvcon, makemkv -ErrorAction SilentlyContinue)
    if ($running.Count -gt 0) { throw "MakeMKV is still running ($($running[0].Name).exe, process id $($running[0].Id))" }

    $mkFlags = switch ($Flags) {
        'default' { Get-MkFlags }
        'nodebug' { @(Get-MkFlags | Where-Object { $_ -ne '--debug' }) }
        'plain'   { @('-r', '--messages=-stdout', '--progress=-same') }
    }
    $open = if ($Target -eq 'bdmv') { "$root\BDMV" } else { $root }
    "scan: $Slug copy at $root; flags=$Flags ($($mkFlags -join ' ')); target=$Target ($open)"
    $start = Get-Date
    & $CAP record --out $out -- $MK @mkFlags info "file:$open" | Out-Null
    $elapsed = [math]::Round(((Get-Date) - $start).TotalSeconds, 1)
    if ($mkFlags -contains '--debug') { Save-MkDebugLog -Since $start -To "$out.makemkv-debug.txt" }
    $exitJson = [string](Get-Content -LiteralPath "$out.exit.json" -Raw -ErrorAction SilentlyContinue)
    $raw = [string](Get-Content -LiteralPath $out -Raw -ErrorAction SilentlyContinue)
    $banner = [regex]::IsMatch($raw, '(?m)^MSG:1005,')
    $count = [regex]::Match($raw, '(?m)^TCOUNT:(\d+)')
    $titles = if ($count.Success) { $count.Groups[1].Value } else { 'NO-TCOUNT-LINE' }
    $msgs = @([regex]::Matches($raw, '(?m)^MSG:(\d+),') | ForEach-Object { $_.Groups[1].Value })
    $last3025 = @([regex]::Matches($raw, '(?m)^MSG:3025,\d+,\d+,"Title #([^ ]+) ') | ForEach-Object { $_.Groups[1].Value })
    "scan: scan-$Name.txt exit.json=$(if ($exitJson) { $exitJson.Trim() } else { 'MISSING' }) seconds=$elapsed version_banner=$banner titles=$titles messages=$($msgs.Count) skipped_titles=$($last3025.Count) last_skipped=$(if ($last3025.Count -gt 0) { $last3025[-1] } else { 'none' })"
    if (-not $exitJson) { throw 'the scan did not finish: exit.json was not written' }
    if (-not $banner) { throw 'MakeMKV did not run: no MSG:1005 version banner' }
    if (-not ($count.Success -and [int]$titles -gt 0)) { throw "the scan lists no titles (titles=$titles)" }
    'RESULT ok'
} catch {
    "RESULT FAILED: $($_.Exception.Message)"
    throw
}
