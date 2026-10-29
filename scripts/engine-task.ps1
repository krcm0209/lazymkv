# Deploys and starts the engine's desktop task, lazymkv-engine (P5; design parts 3.1, 3.5). Run it elevated over SSH
# through scripts/box.sh, after a release build:
#   scripts/box.sh cargo build --workspace --locked --release
#   scripts/box.sh 'powershell -NoProfile -ExecutionPolicy Bypass -File scripts\engine-task.ps1 [-Data DIR] [-AcceptWritableBuild]'
# It registers a task and sets an access list, which are standing changes: run it only after the owner's yes. DIR may
# hold no space or quote. The last line is "ENGINE-TASK-DONE status=ok|error" and the exit code 0 or 1. It writes no
# account name.
param(
    [string]$Data = '',
    [switch]$AcceptWritableBuild
)
$ErrorActionPreference = 'Stop'
$name = 'lazymkv-engine'
$run = 'C:\lazymkv\run'
$release = 'C:\lazymkv\target\release'
# The trees the programs are built from and copied out of (Owner decision 13).
$trees = 'C:\lazymkv\src', 'C:\lazymkv\target'
# lazymkv-ocr.exe arrives with pull request 5 and is copied once it exists.
$required = 'lazymkv-engine.exe', 'lazymkvctl.exe'
$optional = 'lazymkv-ocr.exe'
# SYSTEM, Administrators, and CREATOR OWNER, which grants only to whoever creates an object below.
$trusted = 'S-1-5-18', 'S-1-5-32-544', 'S-1-3-0'
# Ruling W17's write rights (add a file or write data, add a folder or append, delete a child, delete, change the
# access list, take ownership), with GENERIC_WRITE and GENERIC_ALL, which entries for what is created below carry.
$writeMask = 0x500D0046

# The SIDs other than $trusted that own $folder, or that its access list lets write it or anything in it.
function Get-Writers([string]$folder) {
    $acl = Get-Acl -LiteralPath $folder
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

$status = 'error'
try {
    $identity = [System.Security.Principal.WindowsIdentity]::GetCurrent()
    $administrator = [System.Security.Principal.WindowsBuiltInRole]::Administrator
    if (-not (New-Object System.Security.Principal.WindowsPrincipal($identity)).IsInRole($administrator)) {
        throw 'run engine-task.ps1 elevated: it registers a task at the highest run level and protects C:\lazymkv\run (design parts 3.1, 3.5)'
    }
    $task = Get-ScheduledTask -TaskName $name -ErrorAction SilentlyContinue
    if ($task -and $task.State -in 'Running', 'Queued') {
        throw "the task $name is $($task.State); stop its engine with lazymkvctl stop before deploying again (design part 3.1)"
    }
    foreach ($tree in $trees) {
        $writers = @(Get-Writers $tree)
        if ($writers.Count -eq 0) { continue }
        $list = $writers -join ', '
        if (-not $AcceptWritableBuild) {
            throw "$tree lets $list write, and none of them is SYSTEM or Administrators, so a non-elevated process could change a program before it is copied (Owner decision 13); give -AcceptWritableBuild to accept that on this development machine (scripts/box.sh recreates C:\lazymkv\src at every sync, so a list set on it does not last)"
        }
        "accepted: $tree lets $list write (-AcceptWritableBuild, Owner decision 13)"
    }
    foreach ($program in $required) {
        if (-not (Test-Path -LiteralPath (Join-Path $release $program) -PathType Leaf)) {
            throw "$release\$program is missing; build it first with scripts/box.sh cargo build --workspace --locked --release"
        }
    }
    # Design part 3.5's list, folder_sddl's string, set before anything is copied (Ruling T3-24-a).
    $sid = $identity.User.Value
    $dacl = "D:PAI(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;FR;;;$sid)"
    New-Item -ItemType Directory -Force -Path $run | Out-Null
    $protected = New-Object System.Security.AccessControl.DirectorySecurity
    $protected.SetSecurityDescriptorSddlForm("O:BA$dacl", [System.Security.AccessControl.AccessControlSections]'Owner, Access')
    [System.IO.Directory]::SetAccessControl($run, $protected)
    $aces = $dacl.Substring(5)
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
        if (Test-Path -LiteralPath $from -PathType Leaf) { Copy-Item -LiteralPath $from -Destination $run -Force }
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
        $folder = if ($Data) { $Data } else { Join-Path $env:ProgramData 'lazymkv' }
        $result = (Get-ScheduledTaskInfo -TaskName $name).LastTaskResult
        throw "the task started, but lazymkvctl status did not answer within 60 s; the task's last result is $result; read $folder\engine.log"
    }
    $status = 'ok'
} catch {
    "ERROR: $($_.Exception.Message)"
} finally {
    "ENGINE-TASK-DONE status=$status"
}
if ($status -ne 'ok') { exit 1 }
