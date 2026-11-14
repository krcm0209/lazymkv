# The pinned archives' functions of the release (design part 11.5's steps 1 and 2; notes call 8): fetching a file
# whose SHA-256 (Secure Hash Algorithm 256) is pinned, listing and choosing an archive's entries, and fetching
# mkvmerge.exe alone from the MKVToolNix portable archive that packaging/tools.json names. assemble.ps1 dot-sources it
# for its steps 1 and 2, and ffmpeg.yml's smoke job dot-sources it for the mkvmerge.exe its layer 2 tests run, so the
# two fetch and check that archive with the same code (the review of pull request #21 (X8)).
#
#   Get-Sha256 <file>                         the file's SHA-256, lower-case hex.
#   Get-UrlLeaf <url>                         the URL's last segment.
#   Get-Pinned <url> <sha256> <file>          downloads the URL to the file with curl.exe; fails unless the file's
#                                             SHA-256 is the pinned one.
#   Get-ArchiveEntries <archive> <name>       the archive's file entries named exactly <name>, from 7-Zip's listing.
#   Select-TopEntry <archive> <name>          the one entry <top folder>\<name>; fails unless there is exactly one.
#   Get-Mkvmerge <manifest> <downloads> <out> fetches the manifest's portable archive into <downloads>, extracts
#                                             mkvmerge.exe alone into the empty folder <out>, checks its size and its
#                                             version line, and returns its full path.
#
# It only defines functions: it sets no preference and runs nothing when dot-sourced, and each function takes what it
# needs as parameters. Its progress goes to the host, so a function's return value is only what it returns. It needs
# curl.exe and 7z on PATH, and it parses under Windows PowerShell 5.1 as well as PowerShell 7.
# Usage: . (Join-Path <this folder> 'archive.ps1')

# The lower-case hex SHA-256 of a file.
function Get-Sha256([string]$Path) {
    return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

# The last segment of a URL, the name its file is kept under.
function Get-UrlLeaf([string]$Url) {
    return $Url.Substring($Url.LastIndexOf('/') + 1)
}

# fetch: downloads $Url to $File with curl.exe and fails unless its SHA-256 is $Sha256.
function Get-Pinned([string]$Url, [string]$Sha256, [string]$File) {
    Write-Host "fetching $Url"
    & curl.exe --fail --location --silent --show-error --retry 3 --output $File $Url
    if ($LASTEXITCODE -ne 0) { throw "curl.exe could not download $Url (exit $LASTEXITCODE)" }
    $got = Get-Sha256 $File
    if ($got -ne $Sha256) { throw "$Url has SHA-256 $got, not the pinned $Sha256" }
    Write-Host "  SHA-256 $got, as pinned"
}

# The entries of $Archive whose file name is exactly $Name, from 7-Zip's technical listing (7z l -slt): each entry is
# a block of "Key = value" lines after the line "----------", starting with its Path. Folders ("Folder = +", or
# "Attributes = " starting with D) are left out. Each result has Path, with / written \, and Size. A name filter with
# -r matches that name in every folder.
function Get-ArchiveEntries([string]$Archive, [string]$Name) {
    $lines = @(& 7z l -slt $Archive $Name -r)
    if ($LASTEXITCODE -ne 0) { throw "7z could not list $Archive (exit $LASTEXITCODE)" }
    $start = [Array]::IndexOf([string[]]$lines, '----------')
    if ($start -lt 0) { throw "7z l -slt printed no entry listing for $Archive" }
    $entries = @()
    $current = $null
    for ($i = $start + 1; $i -lt $lines.Count; $i++) {
        $line = [string]$lines[$i]
        if ($line.StartsWith('Path = ')) {
            $current = [pscustomobject]@{ Path = $line.Substring(7).Replace('/', '\'); Size = ''; Folder = $false }
            $entries += $current
        } elseif ($null -ne $current -and $line.StartsWith('Size = ')) {
            $current.Size = $line.Substring(7)
        } elseif ($null -ne $current -and $line -eq 'Folder = +') {
            $current.Folder = $true
        } elseif ($null -ne $current -and $line.StartsWith('Attributes = D')) {
            $current.Folder = $true
        }
    }
    return @($entries | Where-Object {
            -not $_.Folder -and $_.Path.Substring($_.Path.LastIndexOf('\') + 1) -ceq $Name
        })
}

# The one entry named $Name directly inside the archive's top folder, "<top>\<Name>", one path separator: every entry
# named $Name is printed with its size first, and anything but exactly one such entry fails with the list.
function Select-TopEntry([string]$Archive, [string]$Name) {
    $named = @(Get-ArchiveEntries $Archive $Name)
    Write-Host "$Archive holds $($named.Count) entries named $($Name):"
    foreach ($entry in $named) { Write-Host "  $($entry.Path) ($($entry.Size) bytes)" }
    $top = @($named | Where-Object { $_.Path.Split('\').Count -eq 2 })
    if ($top.Count -ne 1) {
        $list = ($named | ForEach-Object { $_.Path }) -join ', '
        throw "$Archive has $($top.Count) entries <top folder>\$Name, not one; its entries named $($Name): $list"
    }
    Write-Host "  chosen: $($top[0].Path)"
    return $top[0]
}

# Release step 2's mkvmerge.exe alone, run from $Out outside any installation (ARCH-19). $Manifest is
# packaging/tools.json parsed with ConvertFrom-Json; its MKVToolNix portable archive is fetched into $Downloads under
# its URL's leaf name and checked against the pinned SHA-256. The archive holds one file named mkvmerge.exe, at
# <top folder>\mkvmerge.exe; it is chosen by rule and extracted by its exact path, without -r, so that a future archive
# layout with a second file of that name cannot change which one ships. $Out, made when missing, must be empty and
# must then hold exactly mkvmerge.exe with the listed size, and mkvmerge --version's first line must start
# "mkvmerge v<manifest version> ". Returns the extracted file's full path and nothing else.
function Get-Mkvmerge($Manifest, [string]$Downloads, [string]$Out) {
    $Downloads = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($Downloads)
    $Out = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($Out)
    [void][IO.Directory]::CreateDirectory($Downloads)
    [void][IO.Directory]::CreateDirectory($Out)
    if (@(Get-ChildItem -LiteralPath $Out -Force).Count -ne 0) { throw "the folder $Out for mkvmerge.exe is not empty" }
    $mkvVersion = [string]$Manifest.mkvtoolnix.version
    $url = [string]$Manifest.mkvtoolnix.portable.url
    $portable = Join-Path $Downloads (Get-UrlLeaf $url)
    $null = Get-Pinned $url ([string]$Manifest.mkvtoolnix.portable.sha256) $portable

    $entry = Select-TopEntry $portable 'mkvmerge.exe'
    & 7z e $portable "-o$Out" $entry.Path -y | Out-Host
    if ($LASTEXITCODE -ne 0) { throw "7z could not extract $($entry.Path) from $portable (exit $LASTEXITCODE)" }
    $extracted = @(Get-ChildItem -LiteralPath $Out -Force)
    if ($extracted.Count -ne 1 -or $extracted[0].Name -ne 'mkvmerge.exe') {
        throw "7z extracted $($extracted.Count) entries into $Out, not mkvmerge.exe alone"
    }
    $mkvmerge = [string]$extracted[0].FullName
    if ([string]$extracted[0].Length -ne $entry.Size) {
        throw "$mkvmerge has $($extracted[0].Length) bytes, not the listed $($entry.Size)"
    }
    $mkvmergeSha = Get-Sha256 $mkvmerge
    Write-Host "$mkvmerge from $($entry.Path): SHA-256 $mkvmergeSha, $($extracted[0].Length) bytes"

    $versionLines = @(& $mkvmerge --version)
    if ($LASTEXITCODE -ne 0) { throw "mkvmerge --version exited $LASTEXITCODE" }
    if ($versionLines.Count -eq 0 -or -not ([string]$versionLines[0]).StartsWith("mkvmerge v$mkvVersion ")) {
        $printedLines = $versionLines -join ' | '
        throw "mkvmerge --version printed '$printedLines', not a first line starting 'mkvmerge v$mkvVersion '"
    }
    Write-Host $versionLines[0]
    return $mkvmerge
}
