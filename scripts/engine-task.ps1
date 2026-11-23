# Deploys and starts the engine's desktop task, lazymkv-engine (P5; design parts 3.1, 3.5). Run it elevated over SSH
# through scripts/box.sh, after a release build:
#   scripts/box.sh cargo build --workspace --locked --release
#   scripts/box.sh 'powershell -NoProfile -ExecutionPolicy Bypass -File scripts\engine-task.ps1 [-Data DIR] [-AcceptWritableBuild]'
# or, to run a release zip's own tree rather than the build's three programs (pull request 7's notes, call 10):
#   scripts/box.sh 'powershell -NoProfile -ExecutionPolicy Bypass -File scripts\engine-task.ps1 -Zip FILE -Sha256 HEX [-Data DIR] [-AcceptWritableBuild]'
# where FILE is the release workflow's lazymkv-<version>-windows-x64.zip and HEX the zip's line in the SHA256SUMS the
# same run published. The zip is copied into C:\lazymkv\run, its copy checked against HEX and extracted into
# C:\lazymkv\run\release, and the task runs the tree's bin\lazymkv-engine.exe, which finds its tools in the tree's
# resources folder. It registers a task and sets an access list, which are standing changes: run it only after the owner's yes. DIR is a
# full local folder path below a drive's root, such as D:\lazymkv-data, with no space, quote or percent sign, outside
# C:\lazymkv (neither in the run folder or a build tree nor holding one) and in a folder that exists; the script refuses
# any other before it changes anything. The last line is "ENGINE-TASK-DONE status=ok|error" and the exit code 0 or 1. It
# writes no account name, so it never prints -Zip's own path, which may lie in the owner's profile.
param(
    [string]$Data = '',
    [switch]$AcceptWritableBuild,
    [string]$Zip = '',
    [string]$Sha256 = ''
)
$ErrorActionPreference = 'Stop'
# Expand-Archive's progress bar would only clutter an SSH session's output.
$ProgressPreference = 'SilentlyContinue'
# Windows PowerShell's own modules first: started from a PowerShell 7 session, 5.1 would find PowerShell 7's edition of
# Microsoft.PowerShell.Security first on PSModulePath and Get-Acl would fail to load it (the CI run of review round 1 of
# pull request 3's head).
$env:PSModulePath = "$PSHOME\Modules;$env:PSModulePath"
$name = 'lazymkv-engine'
$run = 'C:\lazymkv\run'
$release = 'C:\lazymkv\target\release'
# The trees the programs are built from and copied out of (Owner decision 13), and the run folder's parent, which a
# writer could rename away to put its own run folder in its place (Ruling T3-24-c).
$trees = (Split-Path $run), 'C:\lazymkv\src', 'C:\lazymkv\target'
# lazymkv-ocr.exe, the OCR worker the engine starts (call 18; T5-R35).
$required = 'lazymkv-engine.exe', 'lazymkvctl.exe', 'lazymkv-ocr.exe'
# SYSTEM, Administrators, and CREATOR OWNER, which grants only to whoever creates an object below.
$trusted = 'S-1-5-18', 'S-1-5-32-544', 'S-1-3-0'
# Ruling W17's write rights (add a file or write data, add a folder or append, delete a child, delete, change the
# access list, take ownership), with GENERIC_WRITE and GENERIC_ALL, which entries for what is created below carry.
$writeMask = 0x500D0046
# A release tree's six files, in the order Get-TreeProblem names the first one missing: the three programs, and the
# three tools the engine finds in <its exe's folder>\..\resources (pull request 7's notes, call 10).
$treeFiles = 'bin\lazymkv-engine.exe', 'bin\lazymkvctl.exe', 'bin\lazymkv-ocr.exe', 'resources\mkvmerge.exe', 'resources\ffprobe.exe', 'resources\ffmpeg.exe'

# The SIDs other than $trusted that own $path, or that its access list lets write it or, for a folder, anything in it.
function Get-Writers([string]$path) {
    $acl = Get-Acl -LiteralPath $path
    $sidType = [System.Security.Principal.SecurityIdentifier]
    $writers = @()
    $owner = $acl.GetOwner($sidType).Value
    if ($trusted -notcontains $owner) { $writers += $owner }
    foreach ($rule in $acl.GetAccessRules($true, $true, $sidType)) {
        $who = $rule.IdentityReference.Value
        $writes = ([int]$rule.FileSystemRights -band $writeMask) -ne 0
        if ($rule.AccessControlType -eq 'Allow' -and $writes -and $trusted -notcontains $who) { $writers += $who }
    }
    $writers | Sort-Object -Unique
}

# Why -Data cannot name the task's data folder, or '' when it can (review round 1 of pull request 3, V13): the
# task's argument line is not quoted, Task Scheduler may expand a percent sign in it, the engine reads a relative
# folder against the run folder and protects whatever folder it is given, a drive's root included, and creates
# only the last component. A folder that holds the run folder or a build tree would put them in the engine's protected
# list, one in a build tree is removed when scripts/box.sh or cargo clean recreates it, and one elsewhere under the run
# folder's parent sits where a non-elevated process may write (the round's final review, W12). $trees are the script's;
# the comparisons end each folder in a backslash, so C:\lazymkv-data passes.
function Get-DataProblem([string]$data, [string]$run, [string[]]$trees) {
    if ($data -notmatch '^[A-Za-z]:\\[^:<>|?*"%\s]*\z') {
        return "-Data $data is not a full local folder path with no space, quote or percent sign, such as D:\lazymkv-data; the task's argument line is not quoted, and the engine reads a relative folder inside $run (design part 3.1)"
    }
    $full = [System.IO.Path]::GetFullPath($data).TrimEnd('\')
    if ($full.Length -le 2) {
        return "-Data $data is a drive's root, which the engine's protected list would reach whole; give a folder below it, such as D:\lazymkv-data (design part 3.5)"
    }
    $ignoreCase = [System.StringComparison]::OrdinalIgnoreCase
    if ($full -eq $run -or $full.StartsWith("$run\", $ignoreCase)) {
        return "-Data $data is in $run, which holds only the programs this script copies there (design part 3.5)"
    }
    foreach ($folder in @($run) + @($trees)) {
        if ($folder.StartsWith("$full\", $ignoreCase)) {
            return "-Data $data holds $folder, so the engine's protected list would reach the programs and the build trees; give a folder outside C:\lazymkv (design part 3.5)"
        }
    }
    # The deepest tree the folder is in, so C:\lazymkv\src\data names C:\lazymkv\src rather than its parent.
    $tree = @($trees | Where-Object { $full -eq $_ -or $full.StartsWith("$_\", $ignoreCase) } | Sort-Object Length -Descending)
    if ($tree.Count -gt 0) {
        if ($tree[0] -eq (Split-Path $run)) {
            return "-Data $data is in $($tree[0]), the run folder's parent, which a non-elevated process may write on this machine (Owner decision 13, Ruling T3-24-c); give a folder outside it (design part 3.5)"
        }
        return "-Data $data is in $($tree[0]), which scripts/box.sh or cargo clean recreates; give a folder outside C:\lazymkv (design part 3.5)"
    }
    if (-not (Test-Path -LiteralPath (Split-Path $full) -PathType Container)) {
        return "-Data $data has no parent folder; the engine creates only the data folder itself (design part 3.5)"
    }
    ''
}

# The data folder the task's engine uses: -Data, or the registry's ProgramData folder joined with lazymkv, read as the
# engine's win::program_data reads it, never from the environment (whole-branch review F2; review round 1 of pull
# request 3, V15).
function Get-DataFolder([string]$data) {
    if ($data) { return $data }
    $key = Get-Item -LiteralPath 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\ProfileList'
    $value = $key.GetValue('ProgramData', '', [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
    $drive = [System.IO.Path]::GetPathRoot([Environment]::SystemDirectory).TrimEnd('\')
    ($value -ireplace '^%SystemDrive%', $drive) + '\lazymkv'
}

# What the script says when lazymkvctl status never answered (review round 1 of pull request 3, V15). A start-up
# refusal reaches only the task's console window at the mini PC's monitor, never engine.log, whose first line start
# writes (design part 3.5). The last result is Task Scheduler's own code (0x0004xxxx, such as 0x41303 "has not yet
# run", 0x8004xxxx, or a 0x8007xxxx launch failure) when the task did not start the engine, and the engine's exit code
# otherwise: 0, 101 for a panic, or a crash code such as 0xC0000005 (the round's final review, W11).
function Get-NoAnswerText([string]$state, [long]$result, [string]$folder, [string]$command) {
    if ($state -eq 'Running') {
        return "the task's engine is running, but lazymkvctl status did not answer within 60 s; run lazymkvctl status again, and read $folder\engine.log"
    }
    if ($result -eq 1 -or $result -eq 2) {
        return "the task's engine exited with $result before lazymkvctl status answered; a start-up refusal goes only to the task's console window at the mini PC's monitor, not to engine.log, so run $command in this elevated console to read it, and stop it with lazymkvctl stop if it starts; a failure after its start is in $folder\engine.log (design parts 3.1, 3.5)"
    }
    $facility = ($result -shr 16) -band 0xFFFF
    if ($facility -in 0x0004, 0x8004, 0x8007) {
        return "the task did not run the engine (its last result is 0x{0:X8}); it runs only while the owner is logged on at the mini PC's own monitor, a locked session being enough (design part 3.1)" -f $result
    }
    # The code is formatted alone, so a brace in -Data's folder or command never reaches -f.
    $code = '{0:X8}' -f $result
    "the task's engine exited with 0x$code before lazymkvctl status answered; a failure after its start is in $folder\engine.log, and a start-up refusal goes only to the task's console window at the mini PC's monitor, so run $command in this elevated console to see it, and stop it with lazymkvctl stop if it starts (design parts 3.1, 3.5)"
}

# The entries of an access list as Windows writes them back, where some SIDs become aliases, the built-in
# Administrator's LA, so the read-back compares like with like (review round 1 of pull request 3, R4).
function Get-WrittenAces([string]$dacl) {
    $security = New-Object System.Security.AccessControl.DirectorySecurity
    $security.SetSecurityDescriptorSddlForm($dacl, [System.Security.AccessControl.AccessControlSections]::Access)
    $written = $security.GetSecurityDescriptorSddlForm([System.Security.AccessControl.AccessControlSections]::Access)
    $written.Substring($written.IndexOf('('))
}

# '' when $zip is a file whose SHA-256 is $sha256 (64 hex digits, either case, since -ne ignores case), else the
# refusal's text (pull request 7's notes, call 10). The script asks it only about the zip's copy in the run folder, so
# nothing can change the file between this check and its extraction, and $zip is never a path in the owner's profile.
function Get-ZipProblem([string]$zip, [string]$sha256) {
    if ($sha256 -notmatch '^[0-9A-Fa-f]{64}\z') {
        return "-Sha256 $sha256 is not 64 hexadecimal digits; the hash is the zip's line in the release workflow's SHA256SUMS"
    }
    if (-not $zip -or -not (Test-Path -LiteralPath $zip -PathType Leaf)) {
        return "$zip is not a file"
    }
    $actual = (Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash
    if ($actual -ne $sha256) {
        return "$zip's SHA-256 is $actual, not -Sha256's $sha256; the hash is the zip's line in the release workflow's SHA256SUMS, and a different one means a different or damaged zip"
    }
    ''
}

# Extracts $zip into the fresh folder $into, an earlier one removed first, and returns the tree's path: the zip's one
# top-level folder, lazymkv-<version>-windows-x64, as packaging/release/smoke.ps1 checks it (pull request 7's notes,
# call 10). It throws for any other top level, so the task never runs from a zip the release workflow did not make.
function Expand-ReleaseTree([string]$zip, [string]$into) {
    if (Test-Path -LiteralPath $into) { Remove-Item -LiteralPath $into -Recurse -Force }
    Expand-Archive -LiteralPath $zip -DestinationPath $into
    $entries = @(Get-ChildItem -LiteralPath $into -Force)
    if ($entries.Count -ne 1 -or -not $entries[0].PSIsContainer -or -not $entries[0].Name.StartsWith('lazymkv-')) {
        throw "the zip holds $($entries.Count) top-level entries, not one folder lazymkv-<version>"
    }
    $entries[0].FullName
}

# '' when $tree holds every one of $treeFiles, else "the release tree lacks" and the first one missing, in their order
# (pull request 7's notes, call 10): the task's programs and the engine's tools must all come from the one zip.
function Get-TreeProblem([string]$tree) {
    foreach ($file in $treeFiles) {
        if (-not (Test-Path -LiteralPath (Join-Path $tree $file) -PathType Leaf)) { return "the release tree lacks $file" }
    }
    ''
}

# Removes the earlier file or folder $path, if any, so what the script puts there is new and inherits the protected
# list (Ruling T3-24-c); a program still running from it keeps it, and the text says what to do then.
function Remove-Earlier([string]$path) {
    if (-not (Test-Path -LiteralPath $path)) { return }
    try { Remove-Item -LiteralPath $path -Recurse -Force -ErrorAction Stop }
    catch { throw "cannot remove the earlier $path ($($_.Exception.Message)); stop any engine running from it with lazymkvctl stop, or remove it by hand, and run the script again (design part 3.5)" }
}

$status = 'error'
try {
    # The zip without its hash would be run unchecked, and the hash without the zip means a deploy the owner did not
    # mean (pull request 7's notes, call 10).
    if (($Zip -eq '') -ne ($Sha256 -eq '')) {
        throw "-Zip and -Sha256 go together: the hash is the zip's line in the release workflow's SHA256SUMS"
    }
    if ($Data) { $problem = Get-DataProblem $Data $run $trees; if ($problem) { throw $problem } }
    $identity = [System.Security.Principal.WindowsIdentity]::GetCurrent()
    $administrator = [System.Security.Principal.WindowsBuiltInRole]::Administrator
    if (-not (New-Object System.Security.Principal.WindowsPrincipal($identity)).IsInRole($administrator)) {
        throw 'run engine-task.ps1 elevated: it registers a task at the highest run level and protects C:\lazymkv\run (design parts 3.1, 3.5)'
    }
    $task = Get-ScheduledTask -TaskName $name -ErrorAction SilentlyContinue
    if ($task -and $task.State -in 'Running', 'Queued') {
        throw "the task $name is $($task.State); stop its engine with lazymkvctl stop before deploying again (design part 3.1)"
    }
    if ($Zip) {
        # With -Zip nothing is copied out of the build folders, so their checks are skipped; the run folder's parent
        # is still checked, since a writer of it could put its own run folder in place of the protected one (Ruling
        # T3-24-c; pull request 7's notes, call 10). The message names no path: -Zip's may lie in the owner's profile.
        if (-not (Test-Path -LiteralPath $Zip -PathType Leaf)) { throw '-Zip names no file; give the release zip itself' }
        $checked = @(Split-Path $run)
    } else {
        foreach ($program in $required) {
            if (-not (Test-Path -LiteralPath (Join-Path $release $program) -PathType Leaf)) {
                throw "$release\$program is missing; build it first with scripts/box.sh cargo build --workspace --locked --release"
            }
        }
        # Owner decision 13's check meets the trees, the release folder and each program copied from it, since an entry
        # set on the folder or on a program alone never shows in a tree's own list (review round 1 of pull request 3, G1).
        $programs = @($required | ForEach-Object { Join-Path $release $_ } | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf })
        $checked = @($trees) + @($release) + $programs
    }
    foreach ($path in $checked) {
        $writers = @(Get-Writers $path)
        if ($writers.Count -eq 0) { continue }
        $list = $writers -join ', '
        if (-not $AcceptWritableBuild) {
            throw "$path lets $list write, and none of them is SYSTEM or Administrators, so a non-elevated process could change a program before it is copied or replace the run folder itself (Owner decision 13); give -AcceptWritableBuild to accept that on this development machine (scripts/box.sh recreates C:\lazymkv\src at every sync, so a list set on it does not last)"
        }
        "accepted: $path lets $list write (-AcceptWritableBuild, Owner decision 13)"
    }
    # Design part 3.5's list, folder_sddl's string, set before anything is copied (Ruling T3-24-a).
    $sid = $identity.User.Value
    $dacl = "D:PAI(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;FR;;;$sid)"
    New-Item -ItemType Directory -Force -Path $run | Out-Null
    # A link would send the access list and the programs to its target, as for the data folder (Rulings T3-3-b, T3-24-c).
    if ((Get-Item -LiteralPath $run -Force).Attributes.HasFlag([System.IO.FileAttributes]::ReparsePoint)) {
        throw "$run is a link, not a folder; remove it by hand and run the script again (design part 3.5)"
    }
    $protected = New-Object System.Security.AccessControl.DirectorySecurity
    $protected.SetSecurityDescriptorSddlForm("O:BA$dacl", [System.Security.AccessControl.AccessControlSections]'Owner, Access')
    [System.IO.Directory]::SetAccessControl($run, $protected)
    $aces = Get-WrittenAces $dacl
    $read = (Get-Acl -LiteralPath $run).Sddl
    if (-not ($read.StartsWith('O:BA') -and ($read.EndsWith("D:PAI$aces") -or $read.EndsWith("D:P$aces")))) {
        throw "$run's access list did not read back as the protected list of design part 3.5"
    }
    # The run folder holds either the three programs or a zip's tree, release, and, while it is checked and extracted,
    # the zip's copy, release.zip; either deploy removes what the other left (T7-R4; pull request 7's notes, call 10).
    $known = @($required) + @('release', 'release.zip')
    $planted = @(Get-ChildItem -LiteralPath $run -Force | Where-Object { $known -notcontains $_.Name })
    if ($planted.Count -gt 0) {
        throw "$run holds $($planted.Name -join ', '), which this script did not put there; remove it by hand and run the script again (design part 3.5)"
    }
    if ($Zip) {
        # The zip is copied first and only the copy is hashed and extracted, so nothing can change it between the check
        # and the extraction; a copy that fails the check is removed before anything earlier is, so a wrong hash leaves
        # the earlier deploy in place (pull request 7's notes, call 10).
        $copy = Join-Path $run 'release.zip'
        $into = Join-Path $run 'release'
        Remove-Earlier $copy
        try { Copy-Item -LiteralPath $Zip -Destination $copy -Force }
        catch { throw "cannot copy the -Zip file to $copy; check that it can be read, and run the script again" }
        $problem = Get-ZipProblem $copy $Sha256
        if ($problem) {
            Remove-Item -LiteralPath $copy -Force
            throw $problem
        }
        foreach ($program in $required) { Remove-Earlier (Join-Path $run $program) }
        Remove-Earlier $into
        try { $tree = Expand-ReleaseTree $copy $into }
        finally { Remove-Item -LiteralPath $copy -Force -ErrorAction SilentlyContinue }
        # Every extracted item owned by Administrators, as packaging/release/smoke.ps1 sets it, since the engine counts
        # any other owner of its tools as a writer and refuses them (pull request 7's notes, call 10).
        & icacls.exe $into /setowner '*S-1-5-32-544' /T /C /Q
        if ($LASTEXITCODE -ne 0) { throw "icacls /setowner on $into exited $LASTEXITCODE" }
        $problem = Get-TreeProblem $tree
        if ($problem) { throw $problem }
        # The tree's folders and files must let no one but SYSTEM and Administrators write, the run folder's protected
        # list inherited; -AcceptWritableBuild never accepts a writer here, since nothing is built (call 10). The two
        # folders above bin are checked too, since a writer of either could rename bin away and put its own in place.
        $bin = Join-Path $tree 'bin'
        $installed = @($into, $tree, $bin, (Join-Path $tree 'resources')) + @($treeFiles | ForEach-Object { Join-Path $tree $_ })
        foreach ($path in $installed) {
            $writers = @(Get-Writers $path)
            if ($writers.Count -gt 0) {
                throw "$path lets $($writers -join ', ') write after the extraction, and none of them is SYSTEM or Administrators, so a non-elevated process could change a program or a tool the engine runs (pull request 7's notes, call 10)"
            }
        }
    } else {
        foreach ($earlier in 'release', 'release.zip') { Remove-Earlier (Join-Path $run $earlier) }
        foreach ($program in $required) {
            $from = Join-Path $release $program
            if (Test-Path -LiteralPath $from -PathType Leaf) {
                # A new file each time, so it inherits the protected list and keeps no earlier file's owner (Ruling T3-24-c).
                $to = Join-Path $run $program
                if (Test-Path -LiteralPath $to) {
                    try { Remove-Item -LiteralPath $to -Force -ErrorAction Stop }
                    catch { throw "cannot remove the earlier $to ($($_.Exception.Message)); stop any engine running from it with lazymkvctl stop, or remove it by hand, and run the script again (design part 3.5)" }
                }
                Copy-Item -LiteralPath $from -Destination $run -Force
            }
        }
        $bin = $run
    }
    # The folder the task's programs run from: the run folder, or the tree's bin with -Zip (call 10).
    $ctl = Join-Path $bin 'lazymkvctl.exe'
    cmd /c "$ctl status >nul 2>&1"
    if ($LASTEXITCODE -eq 0) {
        throw 'an engine already answers on the pipe, such as a console run; stop it with lazymkvctl stop first (design part 3.3)'
    }
    $engine = Join-Path $bin 'lazymkv-engine.exe'
    if ($Data) {
        $action = New-ScheduledTaskAction -Execute $engine -Argument "--data $Data" -WorkingDirectory $bin
    } else {
        $action = New-ScheduledTaskAction -Execute $engine -WorkingDirectory $bin
    }
    # Owner decision 4: no time limit, task priority 7 as phase 0's recordings had, no battery conditions. The engine's
    # console window shows; closing it is a forced shutdown (design part 3.8).
    $settings = New-ScheduledTaskSettingsSet -ExecutionTimeLimit ([TimeSpan]::Zero) -Priority 7 -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -MultipleInstances IgnoreNew
    $principal = New-ScheduledTaskPrincipal -UserId (whoami) -LogonType Interactive -RunLevel Highest
    Register-ScheduledTask -TaskName $name -Action $action -Principal $principal -Settings $settings -Force -ErrorAction Stop | Out-Null
    Start-ScheduledTask -TaskName $name -ErrorAction Stop
    $answered = $false
    for ($try = 1; $try -le 30 -and -not $answered; $try++) {
        Start-Sleep -Seconds 2
        cmd /c "$ctl status >nul 2>&1"
        $answered = $LASTEXITCODE -eq 0
    }
    if (-not $answered) {
        $state = [string](Get-ScheduledTask -TaskName $name).State
        $result = (Get-ScheduledTaskInfo -TaskName $name).LastTaskResult
        $command = if ($Data) { "$engine --data $Data" } else { $engine }
        throw (Get-NoAnswerText $state $result (Get-DataFolder $Data) $command)
    }
    $status = 'ok'
} catch {
    "ERROR: $($_.Exception.Message)"
} finally {
    "ENGINE-TASK-DONE status=$status"
}
if ($status -ne 'ok') { exit 1 }
