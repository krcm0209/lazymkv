# Read-only: what the experiments left on the share and on the mini PC (Task 17). Run it as a job, because the
# share is reachable only from one:  scripts/phase0/job.sh list-leftovers.ps1 final-list
# It creates, changes and deletes nothing. A size is the sum of every file below the folder.
# The last line is "RESULT ok" or "RESULT FAILED: <reason>"; after a failure the script throws.
[CmdletBinding()]
param()
. C:\lazymkv\src\scripts\phase0\env.ps1

# The sum of the sizes of every file below a folder, in bytes (0 for an empty folder).
function Get-FolderBytes([string]$Path) {
    $sum = (Get-ChildItem -LiteralPath $Path -Recurse -File -Force -ErrorAction Stop | Measure-Object -Property Length -Sum).Sum
    if ($null -eq $sum) { return [long]0 }
    return [long]$sum
}

try {
    "staging: $Staging"
    $folders = 0
    foreach ($entry in @(Get-ChildItem -LiteralPath $Staging -Force -ErrorAction Stop | Sort-Object Name)) {
        if ($entry.PSIsContainer) {
            $folders++
            '  folder {0,-46} {1,8:N1} GB' -f $entry.Name, ((Get-FolderBytes $entry.FullName) / 1GB)
            foreach ($item in @(Get-ChildItem -LiteralPath $entry.FullName -Force -ErrorAction Stop | Sort-Object Name)) {
                if ($item.PSIsContainer) {
                    '      folder {0,-40} {1,8:N1} GB' -f $item.Name, ((Get-FolderBytes $item.FullName) / 1GB)
                } else {
                    '      file   {0,-40} {1,12:N0} bytes' -f $item.Name, $item.Length
                }
            }
        } else {
            '  file   {0,-46} {1,12:N0} bytes' -f $entry.Name, $entry.Length
        }
    }
    "staging holds $folders folder(s)"

    if (-not $TestLibrary) {
        'test library: (not set in config.ps1)'
    } elseif (-not (Test-Path -LiteralPath $TestLibrary -PathType Container)) {
        "test library: $TestLibrary (does not exist)"
    } else {
        "test library: $TestLibrary"
        foreach ($file in @(Get-ChildItem -LiteralPath $TestLibrary -Recurse -File -Force -ErrorAction Stop)) {
            '  {0}  {1:N0} bytes' -f $file.FullName, $file.Length
        }
    }

    if (Test-Path -LiteralPath $TuningData -PathType Container) {
        $titles = @(Get-ChildItem -LiteralPath $TuningData -Directory -Force -ErrorAction Stop | ForEach-Object { Get-ChildItem -LiteralPath $_.FullName -Directory -Force -ErrorAction Stop })
        'tuning dataset: {0}: {1:N1} GB in {2} title folder(s)' -f $TuningData, ((Get-FolderBytes $TuningData) / 1GB), $titles.Count
    } else {
        "tuning dataset: $TuningData (does not exist)"
    }

    foreach ($place in "$P0\tmp", "$P0\exp09", $Recordings, "$P0\logs") {
        if (Test-Path -LiteralPath $place) {
            '{0}: {1:N1} MB' -f $place, ((Get-FolderBytes $place) / 1MB)
        } else {
            "${place}: absent"
        }
    }

    $rules = @(Get-NetFirewallRule -DisplayName 'lazymkv-exp05' -ErrorAction SilentlyContinue)
    "firewall rule lazymkv-exp05 (0 = gone): $($rules.Count)"
    'RESULT ok'
} catch {
    "RESULT FAILED: $($_.Exception.Message)"
    throw
}
