# The smoke run of lazymkv's release zip (notes C1 and call 13, Review Focus 5; design parts 3.6 and 11.5). It
# extracts the zip into -Root, makes Administrators (S-1-5-32-544) the owner of the whole tree, gives -Root the
# protected access list D:PAI(A;OICI;FA;;;SY)(A;OICI;FA;;;BA), which lets only SYSTEM and Administrators write,
# with every file and folder below inheriting it (an owner other than those two would count as able to write, W17),
# starts bin\lazymkv-engine.exe --data <Data> hidden with its standard error to a file, waits up to 60 s for
# bin\lazymkvctl.exe status to answer, runs bin\lazymkvctl.exe selftest and reads its JSON (JavaScript Object
# Notation). mkvmerge, ffprobe and ffmpeg must each be ok, found in resources, at the expected version; MakeMKV is not
# required, so the self-test as a whole need not pass. Then bin\lazymkvctl.exe stop, a wait of up to 30 s for the
# engine to exit, and a kill if it has not.
#
# The owner and the list are set with icacls.exe, which ships with every Windows and behaves the same under Windows
# PowerShell 5.1 and PowerShell 7, on a hosted runner (the job runs elevated) and in an administrator's SSH (Secure
# Shell) session: /setowner to a group the elevated token holds needs no extra privilege, and its exit code says
# whether every file took it. Then Get-Acl reads the result back, and the script fails unless -Root's list is
# protected and holds exactly those two entries and every item of the tree is owned by Administrators.
#
# Whatever happens, the engine is not left running, -Data and the engine's standard error file are removed, and the
# last line is exactly "SMOKE-DONE status=ok" (exit 0) or "SMOKE-DONE status=error" (exit 1). -Root is left for the
# caller. The script prints no account name and no host name: access lists are printed in SDDL (Security Descriptor
# Definition Language) form with every S-1-5-21-... SID (security identifier) written <sid>, the engine's and
# lazymkvctl's messages with the account, host and domain names written <account>, <host> and <domain>, and the only
# paths printed are the ones given as parameters and the names under them.
#
# For Windows PowerShell 5.1 (the mini PC, Task 6) and PowerShell 7 (release.yml's package job), run elevated.
# -Root and -Data must not exist; their parent folders must.
# Usage: powershell -NoProfile -File packaging/release/smoke.ps1 -Zip <file> -Root <folder> -Data <folder>
#            [-Mkvmerge 100.0] [-Ffmpeg 9.0.2]
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Zip,
    [Parameter(Mandatory = $true)][string]$Root,
    [Parameter(Mandatory = $true)][string]$Data,
    [string]$Mkvmerge = '100.0',
    [string]$Ffmpeg = '9.0.2'
)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$expectedDacl = @('(A;OICI;FA;;;SY)', '(A;OICI;FA;;;BA)')

function Get-FullPath([string]$Path) {
    return $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($Path)
}

# $Text with every account SID written <sid> and the account, host and domain names written as placeholders.
function Hide-Names([string]$Text) {
    if ($null -eq $Text) { return '' }
    $hidden = [regex]::Replace($Text, 'S-1-5-21(-[0-9]+)+', '<sid>')
    $names = @(@($env:USERNAME, '<account>'), @($env:COMPUTERNAME, '<host>'), @($env:USERDOMAIN, '<domain>'))
    foreach ($pair in $names) {
        $name = [string]$pair[0]
        if ($name.Length -ge 3) {
            $options = [System.Text.RegularExpressions.RegexOptions]::IgnoreCase
            $hidden = [regex]::Replace($hidden, [regex]::Escape($name), $pair[1], $options)
        }
    }
    return $hidden
}

function Write-Line([string]$Text) {
    Write-Host (Hide-Names $Text)
}

# Runs a native program with its output captured as UTF-8 and waits up to $Seconds for it; a program still running
# then is killed and the call fails. Returns its exit code and both streams.
function Invoke-Program([string]$File, [string]$Arguments, [int]$Seconds) {
    $info = New-Object System.Diagnostics.ProcessStartInfo
    $info.FileName = $File
    $info.Arguments = $Arguments
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $info.StandardOutputEncoding = New-Object System.Text.UTF8Encoding -ArgumentList $false
    $info.StandardErrorEncoding = New-Object System.Text.UTF8Encoding -ArgumentList $false
    $process = [System.Diagnostics.Process]::Start($info)
    $stderr = $process.StandardError.ReadToEndAsync()
    $stdout = $process.StandardOutput.ReadToEndAsync()
    if (-not $process.WaitForExit($Seconds * 1000)) {
        try { $process.Kill() } catch { Write-Line "could not kill $File $($Arguments): $($_.Exception.Message)" }
        throw "$File $Arguments did not exit within $Seconds s"
    }
    # The wait without a timeout lets the redirected streams finish.
    $process.WaitForExit()
    return [pscustomobject]@{ Code = $process.ExitCode; Out = $stdout.Result; Err = $stderr.Result }
}

# The owner and access list of one path in SDDL form, names hidden.
function Get-Sddl([string]$Path) {
    $sections = [System.Security.AccessControl.AccessControlSections]::Owner -bor
        [System.Security.AccessControl.AccessControlSections]::Group -bor
        [System.Security.AccessControl.AccessControlSections]::Access
    return Hide-Names (Get-Acl -LiteralPath $Path).GetSecurityDescriptorSddlForm($sections)
}

# Prints the tree's owner and access list: -Root, the tree, resources\ and each tool's file in it.
function Write-Acls([string]$Tree) {
    $paths = @($Root, $Tree, (Join-Path $Tree 'resources'))
    foreach ($tool in @('mkvmerge.exe', 'ffprobe.exe', 'ffmpeg.exe')) { $paths += Join-Path $Tree "resources\$tool" }
    foreach ($path in $paths) {
        if (Test-Path -LiteralPath $path) {
            Write-Line "  $path : $(Get-Sddl $path)"
        } else {
            Write-Line "  $path : missing"
        }
    }
}

# The last lines of the engine's standard error, names hidden.
function Write-EngineErr([string]$Path) {
    if (Test-Path -LiteralPath $Path -PathType Leaf) {
        Write-Line "the engine's standard error, last 40 lines:"
        foreach ($line in @(Get-Content -LiteralPath $Path -Tail 40)) { Write-Line "  $line" }
    }
}

$status = 'error'
$engine = $null
$engineErr = $null
$mayRemove = $false
$tree = $null
try {
    $Zip = Get-FullPath $Zip
    $Root = Get-FullPath $Root
    $Data = Get-FullPath $Data
    $engineErr = "$Data.engine-stderr.txt"
    if (-not (Test-Path -LiteralPath $Zip -PathType Leaf)) { throw "the zip $Zip is not a file" }
    if (Test-Path -LiteralPath $Root) { throw "-Root $Root exists; it must not" }
    if (Test-Path -LiteralPath $Data) { throw "-Data $Data exists; it must not" }
    if (Test-Path -LiteralPath $engineErr) { throw "$engineErr exists; it must not" }
    # The engine's arguments are one string, so -Data is quoted there and may hold no quote or trailing backslash.
    if ($Data.Contains('"') -or $Data.EndsWith('\')) { throw "-Data $Data holds a quote or ends with a backslash" }
    # From here on -Data and the standard error file are this run's, so the cleanup may remove them.
    $mayRemove = $true

    Expand-Archive -LiteralPath $Zip -DestinationPath $Root
    $entries = @(Get-ChildItem -LiteralPath $Root -Force)
    if ($entries.Count -ne 1 -or -not $entries[0].PSIsContainer -or -not $entries[0].Name.StartsWith('lazymkv-')) {
        throw "the zip holds $($entries.Count) top-level entries, not one folder lazymkv-<version>"
    }
    $tree = $entries[0].FullName
    Write-Line "extracted $Zip into $Root ($($entries[0].Name))"
    $engineExe = Join-Path $tree 'bin\lazymkv-engine.exe'
    $ctl = Join-Path $tree 'bin\lazymkvctl.exe'
    foreach ($program in @($engineExe, $ctl)) {
        if (-not (Test-Path -LiteralPath $program -PathType Leaf)) { throw "the tree has no $program" }
    }

    # The owner of every item, then -Root's protected list, then every item below reset to inherit it only.
    & icacls.exe $Root /setowner '*S-1-5-32-544' /T /C /Q
    if ($LASTEXITCODE -ne 0) { throw "icacls /setowner exited $LASTEXITCODE" }
    & icacls.exe $Root /inheritance:r /grant:r '*S-1-5-18:(OI)(CI)F' '*S-1-5-32-544:(OI)(CI)F' /Q
    if ($LASTEXITCODE -ne 0) { throw "icacls /inheritance:r /grant:r exited $LASTEXITCODE" }
    & icacls.exe $tree /reset /T /C /Q
    if ($LASTEXITCODE -ne 0) { throw "icacls /reset exited $LASTEXITCODE" }

    $rootAcl = Get-Acl -LiteralPath $Root
    $dacl = $rootAcl.GetSecurityDescriptorSddlForm([System.Security.AccessControl.AccessControlSections]::Access)
    $match = [regex]::Match($dacl, '^D:([A-Z]*)((\([^)]*\))*)$')
    $aces = @()
    if ($match.Success) {
        $aces = @([regex]::Matches($match.Groups[2].Value, '\([^)]*\)') | ForEach-Object { $_.Value })
    }
    $sameAces = $aces.Count -eq $expectedDacl.Count -and
        @($aces | Where-Object { $expectedDacl -notcontains $_ }).Count -eq 0 -and
        @($expectedDacl | Where-Object { $aces -notcontains $_ }).Count -eq 0
    if (-not $match.Success -or -not $match.Groups[1].Value.Contains('P') -or -not $sameAces) {
        throw "-Root's access list is $(Hide-Names $dacl), not protected with exactly $($expectedDacl -join '')"
    }
    $items = @(Get-Item -LiteralPath $Root) + @(Get-ChildItem -LiteralPath $Root -Recurse -Force)
    $ownerSection = [System.Security.AccessControl.AccessControlSections]::Owner
    foreach ($item in $items) {
        $owner = (Get-Acl -LiteralPath $item.FullName).GetSecurityDescriptorSddlForm($ownerSection)
        if ($owner -ne 'O:BA') { throw "$($item.FullName) has the owner $(Hide-Names $owner), not O:BA" }
    }
    Write-Line "owner O:BA on $($items.Count) items; -Root's access list: $(Hide-Names $dacl)"

    $engine = Start-Process -FilePath $engineExe -ArgumentList ('--data "' + $Data + '"') -WindowStyle Hidden `
        -RedirectStandardError $engineErr -PassThru
    # Reading the handle now keeps the exit code readable after the process ends (Windows PowerShell 5.1).
    $null = $engine.Handle
    Write-Line "started the engine with --data $Data"

    $clock = [System.Diagnostics.Stopwatch]::StartNew()
    $answered = $false
    $last = $null
    while (-not $answered -and $clock.Elapsed.TotalSeconds -lt 60) {
        if ($engine.HasExited) { throw "the engine exited $($engine.ExitCode) before it answered status" }
        $last = Invoke-Program $ctl 'status' 15
        if ($last.Code -eq 0) {
            $answered = $true
        } else {
            Start-Sleep -Milliseconds 500
        }
    }
    if (-not $answered) {
        throw "lazymkvctl status did not answer within 60 s; it last exited $($last.Code): $($last.Err.Trim())"
    }
    Write-Line "the engine answered status after $([int]$clock.Elapsed.TotalSeconds) s"

    $selftest = Invoke-Program $ctl 'selftest' 300
    if ($selftest.Code -ne 0) { throw "lazymkvctl selftest exited $($selftest.Code): $($selftest.Err.Trim())" }
    $checks = @(($selftest.Out | ConvertFrom-Json).checks)
    $expected = @(
        [pscustomobject]@{ Name = 'mkvmerge'; Version = $Mkvmerge },
        [pscustomobject]@{ Name = 'ffprobe'; Version = $Ffmpeg },
        [pscustomobject]@{ Name = 'ffmpeg'; Version = $Ffmpeg }
    )
    $missed = $false
    foreach ($want in $expected) {
        $found = @($checks | Where-Object { $_.name -eq $want.Name })
        if ($found.Count -ne 1) {
            Write-Line "$($want.Name): the self-test has $($found.Count) checks of this name, not one"
            $missed = $true
            continue
        }
        $check = $found[0]
        $line = "$($want.Name): ok=$($check.ok) found=$($check.found) version=$($check.version) detail=$($check.detail)"
        Write-Line $line
        if ($check.ok -ne $true -or $check.found -ne 'resources' -or $check.version -ne $want.Version) {
            Write-Line "  a miss: wanted ok=True found=resources version=$($want.Version)"
            $missed = $true
        }
    }
    if ($missed) {
        Write-Line "the tree's owner and access lists:"
        Write-Acls $tree
        throw 'a tool was not found in resources at its expected version'
    }

    $stop = Invoke-Program $ctl 'stop' 60
    if ($stop.Code -ne 0) { throw "lazymkvctl stop exited $($stop.Code): $($stop.Err.Trim())" }
    if (-not $engine.WaitForExit(30000)) { throw 'the engine did not exit within 30 s of lazymkvctl stop' }
    $engine.WaitForExit()
    if ($engine.ExitCode -ne 0) { throw "the engine exited $($engine.ExitCode) after lazymkvctl stop" }
    Write-Line 'the engine stopped and exited 0'
    $status = 'ok'
} catch {
    Write-Line "smoke: $($_.Exception.Message)"
} finally {
    if ($null -ne $engine) {
        try {
            if (-not $engine.HasExited) {
                Write-Line 'the engine is still running; killing it'
                $engine.Kill()
                if (-not $engine.WaitForExit(30000)) { Write-Line 'the engine did not exit within 30 s of the kill' }
                $status = 'error'
            }
        } catch {
            Write-Line "could not stop the engine: $($_.Exception.Message)"
            $status = 'error'
        }
        if ($status -ne 'ok') { Write-EngineErr $engineErr }
    }
    # -Data must go; the standard error file is only this script's record, so failing to remove it is noted only.
    if ($mayRemove -and (Test-Path -LiteralPath $Data)) {
        try {
            Remove-Item -LiteralPath $Data -Recurse -Force
        } catch {
            Write-Line "could not remove -Data $($Data): $($_.Exception.Message)"
            $status = 'error'
        }
    }
    if ($mayRemove -and (Test-Path -LiteralPath $engineErr)) {
        try {
            Remove-Item -LiteralPath $engineErr -Force
        } catch {
            Write-Line "could not remove $($engineErr): $($_.Exception.Message)"
        }
    }
    Write-Host "SMOKE-DONE status=$status"
}
if ($status -eq 'ok') { exit 0 }
exit 1
