# Assembles lazymkv's Windows release tree, its zip and the release assets (design parts 11.4 and 11.5, P24; notes
# D1 and calls 6 to 8; Rulings T6-R9 and T6-R11). It follows design part 11.5's steps 2 to 6 and stops before the
# release itself: release.yml's package job runs it after step 1's build and after its own shell steps have run the
# release gate (step 3), and only the publish job of a v<version> tag ever makes a draft release of what it writes.
# It calls no bash, since from PowerShell on a Windows runner that name can resolve to the Windows Subsystem for Linux.
#
#   1. Fetches the MKVToolNix portable archive and the three source archives (MKVToolNix, FFmpeg, zlib) named in
#      packaging/tools.json with curl.exe, and fails unless each SHA-256 (Secure Hash Algorithm 256) is the pinned one.
#   2. Extracts mkvmerge.exe alone into resources\ with 7-Zip and runs mkvmerge --version from there: its first line
#      must start "mkvmerge v<manifest version> ". Steps 1 and 2 use archive.ps1 beside this script, which ffmpeg.yml
#      also uses: its Get-Mkvmerge fetches the portable archive and extracts mkvmerge.exe in one call, so step 2 runs
#      before step 1 fetches the three source archives.
#   3. Copies the minimal FFmpeg build's ffmpeg.exe and ffprobe.exe into resources\; the workflow checked their hashes.
#   4. Copies the three programs into bin\ and fails if any of them holds the name VCRUNTIME140 or MSVCP140, read as
#      bytes, since the zip carries no Visual C++ runtime; then writes licenses\ (call 8): lazymkv's LICENSE and the
#      crate notices as bytes; the ocrs models' part's body as lazymkvctl licenses --models writes it to a file;
#      MKVToolNix's COPYING from its checked source archive; FFmpeg's two licence files, zlib's LICENSE, the build
#      settings and the mingw-w64 runtime notice from the FFmpeg artifact; and SOURCES.txt, one
#      "<sha256>  <asset name>" line per source archive.
#   5. Call 8's check, on bytes read as UTF-8: lazymkvctl licenses run from the tree exits 0, prints the committed
#      crates/ctl/crate-notices.txt exactly once, one "Bundled tool licence: <name>" heading for each file of
#      licenses\ that is not one of the three repository texts, and none for those three (T6-R9).
#   6. The signing step, empty for now (design part 11.6): the release is unsigned (P15).
#   7. Zips the tree with 7-Zip into lazymkv-<version>-windows-x64.zip (call 7), has the tree's lazymkvctl
#      export-models write the two model files, each checked against the manifest's SHA-256, copies the other assets
#      and writes SHA256SUMS over every file.
#
# <version> is the lazymkv-engine package's version from cargo metadata, the workspace version. -Bin holds the three
# release programs (target\release), -Ffmpeg the downloaded artifact ffmpeg-windows-x64. -Out must not exist or be
# empty; it ends holding exactly the nine files of the artifact lazymkv-release-dry-run. The tree, the downloads and
# the unpacked archives go to the folder "<Out>-work" beside it, which must not exist. Where bytes matter, a native
# program writes its output to a file through cmd.exe and the script reads the file with [IO.File]::ReadAllBytes,
# because a PowerShell pipeline re-encodes native output and changes its line ends.
#
# PowerShell 7 on the hosted Windows runner only; it needs curl.exe, 7z and cargo on PATH.
# Usage: pwsh -File packaging/release/assemble.ps1 -Bin <dir> -Ffmpeg <dir> -Out <dir>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Bin,
    [Parameter(Mandatory = $true)][string]$Ffmpeg,
    [Parameter(Mandatory = $true)][string]$Out
)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

if ($PSVersionTable.PSVersion.Major -lt 7) {
    throw "assemble.ps1 runs under PowerShell 7 on the release runner, not PowerShell $($PSVersionTable.PSVersion)"
}

# A path made absolute against the current location, whether or not it exists; .NET calls resolve relative paths
# against the process's folder, not PowerShell's, so every path below is absolute.
function Get-FullPath([string]$Path) {
    return $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($Path)
}

function New-Folder([string]$Path) {
    [void][IO.Directory]::CreateDirectory($Path)
}

# Copies one file byte for byte; the source must exist and the target must not.
function Copy-Bytes([string]$From, [string]$To) {
    if (-not (Test-Path -LiteralPath $From -PathType Leaf)) { throw "$From is not a file" }
    [IO.File]::Copy($From, $To, $false)
}

# The cmd.exe line of Write-NativeOutput is not quoted, so a path that holds a space or one of cmd's special
# characters is refused instead of being split or read as an operator.
function Assert-PlainPath([string]$Path) {
    if ($Path.IndexOfAny([char[]]' "&|<>^%!()') -ge 0) {
        throw "the path $Path holds a space or a character cmd.exe reads specially; run from a plain folder"
    }
}

# Runs "<Program> <Arguments> > <File>" through cmd.exe, so the program's standard output reaches the file as bytes,
# and fails unless it exits 0. The working folder is PowerShell's current location.
function Write-NativeOutput([string]$Program, [string]$Arguments, [string]$File) {
    Assert-PlainPath $Program
    Assert-PlainPath $File
    & cmd.exe /d /c "$Program $Arguments > $File"
    if ($LASTEXITCODE -ne 0) { throw "$Program $Arguments exited $LASTEXITCODE" }
    if (-not (Test-Path -LiteralPath $File -PathType Leaf)) { throw "$Program $Arguments wrote no $File" }
}

# The number of times $Needle occurs in $Haystack, compared ordinally, without overlaps.
function Get-Count([string]$Haystack, [string]$Needle) {
    $count = 0
    $at = $Haystack.IndexOf($Needle, [StringComparison]::Ordinal)
    while ($at -ge 0) {
        $count++
        $at = $Haystack.IndexOf($Needle, $at + $Needle.Length, [StringComparison]::Ordinal)
    }
    return $count
}

# UTF-8 that writes no byte order mark and throws on an invalid byte, so a decoded text is exactly its bytes.
$utf8 = New-Object System.Text.UTF8Encoding -ArgumentList $false, $true

function Read-Utf8([string]$Path) {
    return $utf8.GetString([IO.File]::ReadAllBytes($Path))
}

# The pinned archives' functions, shared with ffmpeg.yml's smoke job: Get-Sha256, Get-UrlLeaf, Get-Pinned,
# Get-ArchiveEntries, Select-TopEntry and Get-Mkvmerge (the review of pull request #21 (X8)).
. (Join-Path $PSScriptRoot 'archive.ps1')

$Bin = Get-FullPath $Bin
$Ffmpeg = Get-FullPath $Ffmpeg
$Out = Get-FullPath $Out
$repo = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$manifest = [IO.File]::ReadAllText((Join-Path $repo 'packaging\tools.json')) | ConvertFrom-Json

# The workspace version (call 7): lazymkv-engine's version.workspace, read with cargo metadata.
$cargoToml = Join-Path $repo 'Cargo.toml'
$metadataLines = @(& cargo metadata --manifest-path $cargoToml --no-deps --format-version 1 --locked)
if ($LASTEXITCODE -ne 0) { throw "cargo metadata exited $LASTEXITCODE" }
$engine = @(($metadataLines -join "`n" | ConvertFrom-Json).packages | Where-Object { $_.name -eq 'lazymkv-engine' })
if ($engine.Count -ne 1) { throw "cargo metadata lists $($engine.Count) packages named lazymkv-engine, not one" }
$version = [string]$engine[0].version
if ($version -notmatch '^[0-9]+\.[0-9]+\.[0-9]+$') { throw "the workspace version '$version' is not <n>.<n>.<n>" }
Write-Host "workspace version: $version"

$programs = @('lazymkv-engine.exe', 'lazymkv-ocr.exe', 'lazymkvctl.exe')
foreach ($program in $programs) {
    if (-not (Test-Path -LiteralPath (Join-Path $Bin $program) -PathType Leaf)) { throw "$Bin holds no $program" }
}
$artifactFiles = @('ffmpeg.exe', 'ffprobe.exe', 'COPYING.LGPLv2.1', 'LICENSE.md', 'LICENSE',
    'ffmpeg-build-settings.txt', 'mingw-w64-runtime-copyright.txt')
foreach ($file in $artifactFiles) {
    if (-not (Test-Path -LiteralPath (Join-Path $Ffmpeg $file) -PathType Leaf)) { throw "$Ffmpeg holds no $file" }
}

if (Test-Path -LiteralPath $Out) {
    if (@(Get-ChildItem -LiteralPath $Out -Force).Count -ne 0) { throw "the out folder $Out is not empty" }
} else {
    New-Folder $Out
}
$work = Join-Path (Split-Path -Parent $Out) ((Split-Path -Leaf $Out) + '-work')
if (Test-Path -LiteralPath $work) { throw "the work folder $work exists; it must not" }
$downloads = Join-Path $work 'downloads'
$unpacked = Join-Path $work 'unpacked'
$treeName = "lazymkv-$version"
$tree = Join-Path $work $treeName
$treeBin = Join-Path $tree 'bin'
$resources = Join-Path $tree 'resources'
$licenses = Join-Path $tree 'licenses'
foreach ($folder in @($downloads, $unpacked, $treeBin, $resources, $licenses)) { New-Folder $folder }

# Steps 1 and 2 for the portable archive, by archive.ps1's Get-Mkvmerge (the review of pull request #21 (X8)): the
# archive is fetched into downloads\ and checked before anything reads it, and mkvmerge.exe alone is extracted into
# resources\, where its size and its --version line are checked, to run from there outside any installation (ARCH-19).
$mkvmerge = Get-Mkvmerge $manifest $downloads $resources
if (@($mkvmerge).Count -ne 1 -or -not (Test-Path -LiteralPath $mkvmerge -PathType Leaf)) {
    throw "Get-Mkvmerge returned '$mkvmerge', not the one file resources\mkvmerge.exe"
}

# Step 1 for the three source archives, each checked before anything reads it.
$mkvVersion = [string]$manifest.mkvtoolnix.version
$sources = @(
    [pscustomobject]@{ Pin = $manifest.mkvtoolnix.source; Name = "mkvtoolnix-$mkvVersion.tar.xz" },
    [pscustomobject]@{ Pin = $manifest.ffmpeg.source; Name = "ffmpeg-$($manifest.ffmpeg.version).tar.xz" },
    [pscustomobject]@{ Pin = $manifest.ffmpeg.zlib; Name = "zlib-$($manifest.ffmpeg.zlib.version).tar.xz" }
)
foreach ($source in $sources) {
    $leaf = Get-UrlLeaf $source.Pin.url
    if ($leaf -ne $source.Name) { throw "the manifest's URL $($source.Pin.url) names $leaf, not $($source.Name)" }
    Get-Pinned $source.Pin.url $source.Pin.sha256 (Join-Path $downloads $source.Name)
}

# Step 3: the two FFmpeg programs.
foreach ($program in @('ffmpeg.exe', 'ffprobe.exe')) {
    Copy-Bytes (Join-Path $Ffmpeg $program) (Join-Path $resources $program)
}

# Step 4: the programs, then licenses\.
foreach ($program in $programs) {
    Copy-Bytes (Join-Path $Bin $program) (Join-Path $treeBin $program)
}

# None of the three programs may import the Visual C++ runtime, which the zip does not carry and Windows does not
# ship; .cargo/config.toml links it statically (the review of pull request #21 (X1)). The check reads the import
# names as bytes, each file decoded one character per byte (ISO 8859-1), since the runner has no dumpbin on PATH.
$latin1 = [Text.Encoding]::GetEncoding(28591)
foreach ($program in $programs) {
    $programText = $latin1.GetString([IO.File]::ReadAllBytes((Join-Path $treeBin $program)))
    foreach ($runtime in @('VCRUNTIME140', 'MSVCP140')) {
        if ($programText.IndexOf($runtime, [StringComparison]::OrdinalIgnoreCase) -ge 0) {
            throw "bin\$program holds the name $runtime, so it imports the Visual C++ runtime the zip does not carry"
        }
    }
}
Write-Host 'bin\: none of the three programs imports VCRUNTIME140 or MSVCP140'

Copy-Bytes (Join-Path $repo 'LICENSE') (Join-Path $licenses 'lazymkv-LICENSE.txt')
$noticesPath = Join-Path $repo 'crates\ctl\crate-notices.txt'
Copy-Bytes $noticesPath (Join-Path $licenses 'crate-notices.txt')

# The models' part's body, attribution then text with its final newline: the file holds exactly the bytes lazymkvctl
# licenses --models writes to its standard output, so it is the text that licenses prints under the part's heading and
# compares a tree's licenses\ocrs-models.txt with (the review of pull request #21 (X9)).
$modelsText = Join-Path $licenses 'ocrs-models.txt'
Write-NativeOutput (Join-Path $Bin 'lazymkvctl.exe') 'licenses --models' $modelsText
if ((Get-Item -LiteralPath $modelsText).Length -eq 0) {
    throw "lazymkvctl licenses --models wrote an empty $modelsText"
}

# MKVToolNix's COPYING from its checked source archive: 7-Zip takes off the xz layer, then reads the tar, since
# Windows' own tar reads no xz. The source tree holds more than one file named COPYING (bundled libraries carry their
# own), so the one at <top folder>\COPYING is chosen by rule and extracted by its exact path.
$mkvSource = Join-Path $downloads "mkvtoolnix-$mkvVersion.tar.xz"
$xzOut = Join-Path $unpacked 'mkvtoolnix-tar'
$tarOut = Join-Path $unpacked 'mkvtoolnix'
& 7z e $mkvSource "-o$xzOut" -y
if ($LASTEXITCODE -ne 0) { throw "7z could not read the xz layer of $mkvSource (exit $LASTEXITCODE)" }
$tars = @(Get-ChildItem -LiteralPath $xzOut -File)
if ($tars.Count -ne 1) { throw "7z wrote $($tars.Count) files from the xz layer of $mkvSource, not one tar" }
$copyingEntry = Select-TopEntry $tars[0].FullName 'COPYING'
if ($copyingEntry.Path -ne "mkvtoolnix-$mkvVersion\COPYING") {
    throw "the MKVToolNix source's top-level COPYING is $($copyingEntry.Path), not mkvtoolnix-$mkvVersion\COPYING"
}
& 7z x $tars[0].FullName "-o$tarOut" $copyingEntry.Path -y
if ($LASTEXITCODE -ne 0) { throw "7z could not extract $($copyingEntry.Path) (exit $LASTEXITCODE)" }
$copying = Join-Path $tarOut $copyingEntry.Path
$extracted = @(Get-ChildItem -LiteralPath $tarOut -Recurse -File)
if ($extracted.Count -ne 1 -or -not (Test-Path -LiteralPath $copying -PathType Leaf)) {
    throw "7z extracted $($extracted.Count) files into $tarOut, not $($copyingEntry.Path) alone"
}
Write-Host "mkvtoolnix-COPYING.txt from $($copyingEntry.Path): SHA-256 $(Get-Sha256 $copying)"
Copy-Bytes $copying (Join-Path $licenses 'mkvtoolnix-COPYING.txt')

# The FFmpeg artifact's licence files (build.sh names zlib's file LICENSE).
Copy-Bytes (Join-Path $Ffmpeg 'COPYING.LGPLv2.1') (Join-Path $licenses 'ffmpeg-COPYING.LGPLv2.1.txt')
Copy-Bytes (Join-Path $Ffmpeg 'LICENSE.md') (Join-Path $licenses 'ffmpeg-LICENSE.md')
Copy-Bytes (Join-Path $Ffmpeg 'ffmpeg-build-settings.txt') (Join-Path $licenses 'ffmpeg-build-settings.txt')
Copy-Bytes (Join-Path $Ffmpeg 'LICENSE') (Join-Path $licenses 'zlib-LICENSE.txt')
Copy-Bytes (Join-Path $Ffmpeg 'mingw-w64-runtime-copyright.txt') (Join-Path $licenses 'mingw-w64-runtime-copyright.txt')

$sourceLines = ''
foreach ($source in $sources) {
    $sourceLines += "$(Get-Sha256 (Join-Path $downloads $source.Name))  $($source.Name)`n"
}
[IO.File]::WriteAllBytes((Join-Path $licenses 'SOURCES.txt'), $utf8.GetBytes($sourceLines))
Write-Host "SOURCES.txt:"
Write-Host -NoNewline $sourceLines

# Step 5: call 8's check, lazymkvctl licenses run from the assembled tree (T6-R9).
$repositoryTexts = @('lazymkv-LICENSE.txt', 'ocrs-models.txt', 'crate-notices.txt')
$bundledTexts = @('mkvtoolnix-COPYING.txt', 'ffmpeg-COPYING.LGPLv2.1.txt', 'ffmpeg-LICENSE.md',
    'ffmpeg-build-settings.txt', 'zlib-LICENSE.txt', 'mingw-w64-runtime-copyright.txt', 'SOURCES.txt')
$present = @(Get-ChildItem -LiteralPath $licenses -Force | ForEach-Object { $_.Name } | Sort-Object)
$listed = @(($repositoryTexts + $bundledTexts) | Sort-Object)
if (($present -join '|') -ne ($listed -join '|')) {
    throw "licenses\ holds '$($present -join ', ')', not the ten files '$($listed -join ', ')'"
}
$check = Join-Path $work 'licenses-from-tree.txt'
Push-Location -LiteralPath $tree
try {
    Write-NativeOutput 'bin\lazymkvctl.exe' 'licenses' $check
} finally {
    Pop-Location
}
$checkText = Read-Utf8 $check
$notices = Get-Count $checkText (Read-Utf8 $noticesPath)
if ($notices -ne 1) { throw "lazymkvctl licenses from the tree printed crates/ctl/crate-notices.txt $notices times" }
$problems = @()
foreach ($name in $repositoryTexts) {
    $count = Get-Count $checkText "`nBundled tool licence: $name`n"
    if ($count -ne 0) { $problems += "$name has $count bundled headings, not 0" }
}
foreach ($name in $bundledTexts) {
    $count = Get-Count $checkText "`nBundled tool licence: $name`n"
    if ($count -ne 1) { $problems += "$name has $count bundled headings, not 1" }
}
$headings = Get-Count $checkText "`nBundled tool licence: "
if ($headings -ne $bundledTexts.Count) { $problems += "$headings bundled headings in all, not $($bundledTexts.Count)" }
if ($problems.Count -ne 0) { throw "lazymkvctl licenses from the tree: $($problems -join '; ')" }
Write-Host "call 8: the crate notices printed once, $headings bundled headings, none for the three repository texts"

# Step 6: the signing step (design part 11.6).
Write-Host 'signing: the release is unsigned (P15)'

# Step 7: the zip, the model files, the other assets and SHA256SUMS.
$zipName = "lazymkv-$version-windows-x64.zip"
$zip = Join-Path $Out $zipName
& 7z a -tzip $zip $tree -y
if ($LASTEXITCODE -ne 0) { throw "7z could not write $zip (exit $LASTEXITCODE)" }
& (Join-Path $treeBin 'lazymkvctl.exe') export-models $Out
if ($LASTEXITCODE -ne 0) { throw "lazymkvctl export-models exited $LASTEXITCODE" }
foreach ($model in @('text-detection.onnx', 'text-recognition.onnx')) {
    $want = [string]$manifest.models.$model.sha256
    $got = Get-Sha256 (Join-Path $Out $model)
    if ($got -ne $want) { throw "export-models wrote $model with SHA-256 $got, not the manifest's $want" }
}
foreach ($source in $sources) {
    Copy-Bytes (Join-Path $downloads $source.Name) (Join-Path $Out $source.Name)
}
Copy-Bytes (Join-Path $Ffmpeg 'ffmpeg-build-settings.txt') (Join-Path $Out 'ffmpeg-build-settings.txt')
Copy-Bytes $modelsText (Join-Path $Out 'ocrs-models.txt')

[string[]]$wanted = @($zipName) + @($sources | ForEach-Object { $_.Name }) +
    @('ffmpeg-build-settings.txt', 'text-detection.onnx', 'text-recognition.onnx', 'ocrs-models.txt')
[string[]]$names = @(Get-ChildItem -LiteralPath $Out -Force | ForEach-Object { $_.Name })
[Array]::Sort($names, [StringComparer]::Ordinal)
[Array]::Sort($wanted, [StringComparer]::Ordinal)
if (($names -join '|') -ne ($wanted -join '|')) {
    throw "$Out holds '$($names -join ', ')', not the eight assets '$($wanted -join ', ')'"
}
$sums = ''
foreach ($name in $names) {
    $sums += "$(Get-Sha256 (Join-Path $Out $name))  $name`n"
}
[IO.File]::WriteAllBytes((Join-Path $Out 'SHA256SUMS'), $utf8.GetBytes($sums))
Write-Host "SHA256SUMS:"
Write-Host -NoNewline $sums
Write-Host "the zip: $((Get-Item -LiteralPath $zip).Length) bytes"
