# Deploys and starts the engine's desktop task, lazymkv-engine (P5; design parts 3.1, 3.5). Run it elevated over SSH
# through scripts/box.sh, after a release build:
#   scripts/box.sh cargo build --workspace --locked --release
#   scripts/box.sh 'powershell -NoProfile -ExecutionPolicy Bypass -File scripts\engine-task.ps1 [-Data DIR] [-AcceptWritableBuild]'
# It registers a task and sets an access list, which are standing changes: run it only after the owner's yes. DIR is a
# full local folder path below a drive's root, such as D:\lazymkv-data, with no space, quote or percent sign, outside
# C:\lazymkv (neither in the run folder or a build tree nor holding one) and in a folder that exists; the script refuses
# any other before it changes anything. The last line is "ENGINE-TASK-DONE status=ok|error" and the exit code 0 or 1. It
# writes no account name.
param(
    [string]$Data = '',
    [switch]$AcceptWritableBuild
)
$ErrorActionPreference = 'Stop'
$name = 'lazymkv-engine'
$run = 'C:\lazymkv\run'
$release = 'C:\lazymkv\target\release'
# The trees the programs are built from and copied out of (Owner decision 13), and the run folder's parent, which a
# writer could rename away to put its own run folder in its place (Ruling T3-24-c).
$trees = (Split-Path $run), 'C:\lazymkv\src', 'C:\lazymkv\target'
# lazymkv-ocr.exe arrives with pull request 5 and is copied once it exists.
$required = 'lazymkv-engine.exe', 'lazymkvctl.exe'
$optional = 'lazymkv-ocr.exe'
# SYSTEM, Administrators, and CREATOR OWNER, which grants only to whoever creates an object below.
$trusted = 'S-1-5-18', 'S-1-5-32-544', 'S-1-3-0'
# Ruling W17's write rights (add a file or write data, add a folder or append, delete a child, delete, change the
# access list, take ownership), with GENERIC_WRITE and GENERIC_ALL, which entries for what is created below carry.
$writeMask = 0x500D0046

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
# list, and one in a build tree is removed when scripts/box.sh or cargo clean recreates it (the round's final review,
# W12). $trees are the script's; the comparisons end each folder in a backslash, so C:\lazymkv-data passes.
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

$status = 'error'
try {
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
    foreach ($program in $required) {
        if (-not (Test-Path -LiteralPath (Join-Path $release $program) -PathType Leaf)) {
            throw "$release\$program is missing; build it first with scripts/box.sh cargo build --workspace --locked --release"
        }
    }
    # Owner decision 13's check meets the trees, the release folder and each program copied from it, since an entry set
    # on the folder or on a program alone never shows in a tree's own list (review round 1 of pull request 3, G1).
    $programs = @($required + $optional | ForEach-Object { Join-Path $release $_ } | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf })
    foreach ($path in @($trees) + @($release) + $programs) {
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
    $known = $required + $optional
    $planted = @(Get-ChildItem -LiteralPath $run -Force | Where-Object { $known -notcontains $_.Name })
    if ($planted.Count -gt 0) {
        throw "$run holds $($planted.Name -join ', '), which this script did not put there; remove it by hand and run the script again (design part 3.5)"
    }
    foreach ($program in $known) {
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
    $ctl = Join-Path $run 'lazymkvctl.exe'
    cmd /c "$ctl status >nul 2>&1"
    if ($LASTEXITCODE -eq 0) {
        throw 'an engine already answers on the pipe, such as a console run; stop it with lazymkvctl stop first (design part 3.3)'
    }
    $engine = Join-Path $run 'lazymkv-engine.exe'
    if ($Data) {
        $action = New-ScheduledTaskAction -Execute $engine -Argument "--data $Data" -WorkingDirectory $run
    } else {
        $action = New-ScheduledTaskAction -Execute $engine -WorkingDirectory $run
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
