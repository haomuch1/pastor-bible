# Run the INSTALLED app as a machine that has never seen this repository.
#
# P7 found the bug this exists to catch. paths.rs resolved nine runtime files
# through repo_root(), which is env!("CARGO_MANIFEST_DIR") -- an absolute path
# on whoever ran the compiler. Every P6 install check ran the installed program
# on this machine, where that path is a real directory full of the right files,
# so the installed app quietly read the repository and every check passed. On a
# laptop that had never seen the repository, the first of those reads failed and
# the app opened to:
#
#   cannot read disclaimer.txt: The system cannot find the path specified (os error 3)
#
# The only honest way to test an installed program is to take away everything a
# stranger would not have. So this renames the repository directory, clears
# every TPB_* variable, and runs the installed binary's --self-check, which
# performs exactly the reads that failed and exits non-zero naming any that
# still do. The rename is undone in a finally block, and the restoration is
# verified again afterwards, so a failure to put the repository back is itself
# a failure of this script.
#
#   powershell -ExecutionPolicy Bypass -File tools\clean-machine-check.ps1
#
# Add -Interactive to also open the window and stop for a human to confirm it
# reaches the main screen. --self-check cannot see the window, only the reads
# behind it.
#
# Windows refuses to rename a directory that any process has open -- an editor
# with a file from it, a shell sitting in it -- and that is the normal state on
# a machine somebody works on. Two fallbacks, in descending strength:
#
#   (none)         rename the repository itself. The real thing. Needs nothing
#                  holding the directory, so run it from a shell that is not in
#                  it, with editors closed.
#   -HideContents  rename every top-level entry instead, leaving an empty
#                  directory behind. Nothing has a *child* as its current
#                  directory, so this works while a shell sits in the root. The
#                  app then finds nothing at any repository path, which is the
#                  same claim; all that survives is an empty directory node.
#                  .git is left alone: the app never reads it.
#   -DataOnly      rename only data/, where all nine files that broke lived.
#                  Proves the app no longer reads data/, not that it reads
#                  nothing from the repository at all.
#
# Nothing here writes to the repository. It needs the app installed already.

[CmdletBinding()]
param(
    [switch] $Interactive,
    [switch] $DataOnly,
    [switch] $HideContents,
    [int] $TimeoutSeconds = 60
)

$ErrorActionPreference = 'Stop'

$repo    = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$parent  = Split-Path $repo -Parent
$leaf    = Split-Path $repo -Leaf
$SUFFIX = '.hidden-by-clean-machine-check'
if ($DataOnly -and $HideContents) { throw "-DataOnly and -HideContents are different checks; pick one." }
if ($DataOnly) {
    $mode   = 'one'
    $target = Join-Path $repo 'data'
    $hidden = Join-Path $repo ('data' + $SUFFIX)
    $what   = 'data/'
} elseif ($HideContents) {
    $mode   = 'many'
    $target = $repo
    $what   = "everything in the repository"
} else {
    $mode   = 'one'
    $target = $repo
    $hidden = Join-Path $parent ($leaf + $SUFFIX)
    $what   = 'the repository'
}
$install = Join-Path $env:LOCALAPPDATA 'The Pastor Bible'
$exe     = Join-Path $install 'pastor-bible.exe'
$appData = Join-Path $env:APPDATA 'io.github.haomuch1.pastorbible'

$report  = Join-Path $appData 'self-check.txt'

$script:failed = $false
function Fail($msg) { Write-Host "FAIL  $msg" -ForegroundColor Red; $script:failed = $true }
function Pass($msg) { Write-Host "pass  $msg" -ForegroundColor Green }

if (-not (Test-Path $exe))    { throw "no installed app at $exe. Install one first." }
if (-not (Test-Path $target)) { throw "$target is not there to hide." }
if ($mode -eq 'many') {
    $leftovers = @(Get-ChildItem -LiteralPath $repo -Force | Where-Object Name -like "*$SUFFIX")
    if ($leftovers.Count) { throw "a previous run left: $($leftovers.Name -join ', ')" }
} elseif (Test-Path $hidden) {
    throw "$hidden already exists; a previous run did not clean up."
}

# Every TPB_* override off. A variable left set would let the app find the
# repository by another route, and the run would prove nothing.
Get-ChildItem env: | Where-Object Name -like 'TPB_*' | ForEach-Object {
    Write-Host "  clearing $($_.Name)"
    Remove-Item "env:$($_.Name)" -ErrorAction SilentlyContinue
}
$still = @(Get-ChildItem env: | Where-Object Name -like 'TPB_*')
if ($still.Count -gt 0) { throw "TPB_* variables survived clearing: $($still.Name -join ', ')" }
Pass "no TPB_* variables are set"

# A report left by an earlier run would be read as this run's answer.
Remove-Item $report -ErrorAction SilentlyContinue
if (Test-Path $report) { throw "cannot clear the previous report at $report" }

# Windows will not rename a directory any process has as its current
# directory, and this script lives inside the one being renamed. Step out
# first. The script is already loaded, so moving the file under it is fine.
Set-Location -LiteralPath $parent

$moved = $false
$renamed = @()
try {
    Write-Host "hiding $what  ($target)"
    if ($mode -eq 'many') {
        # .git is not something the app can read, and renaming it under a live
        # git checkout invites trouble for no gain.
        foreach ($c in Get-ChildItem -LiteralPath $repo -Force | Where-Object Name -ne '.git') {
            $to = Join-Path $repo ($c.Name + $SUFFIX)
            Move-Item -LiteralPath $c.FullName -Destination $to -ErrorAction Stop
            $renamed += ,@($to, $c.FullName)
        }
        $moved = $true
        $left = @(Get-ChildItem -LiteralPath $repo -Force | Where-Object { $_.Name -ne '.git' -and $_.Name -notlike "*$SUFFIX" })
        if ($left.Count) { throw "these were not hidden: $($left.Name -join ', ')" }
        Pass "the repository holds nothing but an empty directory and .git ($($renamed.Count) entries hidden)"
    } else {
        try {
            Move-Item -LiteralPath $target -Destination $hidden -ErrorAction Stop
        } catch {
            throw ("cannot rename $target : $($_.Exception.Message)`n" +
                   "Something is holding it open. Close editors, shells and file " +
                   "managers sitting in that directory and run this again from " +
                   "outside it, or pass -HideContents, which renames the entries " +
                   "inside instead and makes the same claim.")
        }
        $moved = $true
        if (Test-Path $target) { throw "$target is still there after the move" }
        Pass "$what is not on disk at its build-time path"
    }

    Write-Host ""
    Write-Host "--- $exe --self-check ---"
    # A release build is a GUI-subsystem binary: it has no console, so calling
    # it directly returns at once, prints nothing and sets no exit code. Start
    # it and wait for the handle, then read the report it wrote.
    # Bounded: a build older than --self-check ignores the argument and opens a
    # window, which never exits. Waiting for that forever is not a test result.
    $p = Start-Process $exe -ArgumentList '--self-check' -PassThru
    if ($p.WaitForExit($TimeoutSeconds * 1000)) {
        $code = $p.ExitCode
    } else {
        $code = -1
        Write-Host "  it did not exit within $TimeoutSeconds seconds; stopping it"
        Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
    }
    if (Test-Path $report) {
        Get-Content $report | ForEach-Object { Write-Host "  $_" }
    } else {
        Write-Host "  (no report at $report)"
    }
    Write-Host "--- exit $code ---"
    Write-Host ""
    if ($code -eq 0 -and (Test-Path $report)) {
        Pass "the installed app resolved everything it needs with $what gone"
    } elseif (-not (Test-Path $report)) {
        # An older build has no --self-check: it ignores the argument and opens
        # a window instead, which never exits and leaves no report. That is a
        # failure of this check, and worth saying plainly rather than as a code.
        Fail "the installed app wrote no report. It is probably older than --self-check (v1.0.1); it may have opened a window instead."
        Get-Process -Name 'pastor-bible' -EA SilentlyContinue | Stop-Process -Force -EA SilentlyContinue
    } else {
        Fail "--self-check exited $code"
    }

    if ($Interactive) {
        Write-Host "starting the window..."
        $p = Start-Process $exe -PassThru
        $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
        while ((Get-Date) -lt $deadline -and -not $p.HasExited) { Start-Sleep -Milliseconds 500 }
        if ($p.HasExited) { Fail "the app exited with code $($p.ExitCode)" }
        Write-Host ""
        Write-Host "Look at the window. It must show the welcome or main screen," -ForegroundColor Yellow
        Write-Host "with the disclaimer and the crisis note -- NOT 'The Pastor" -ForegroundColor Yellow
        Write-Host "Bible could not start'. Press Enter when you have looked." -ForegroundColor Yellow
        Read-Host | Out-Null
        if (-not $p.HasExited) { Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue }
    }
}
finally {
    if ($moved -and $mode -eq 'many') {
        Write-Host "restoring $($renamed.Count) entries"
        foreach ($pair in $renamed) {
            try { Move-Item -LiteralPath $pair[0] -Destination $pair[1] -ErrorAction Stop }
            catch { Write-Host "COULD NOT RESTORE $($pair[0]) -> $($pair[1]): $($_.Exception.Message)" -ForegroundColor Red }
        }
    } elseif ($moved) {
        Write-Host "restoring $target"
        Move-Item -LiteralPath $hidden -Destination $target
    }
}

# Verified outside the finally: putting it back is part of passing.
$proof = if ($DataOnly) { Join-Path $repo 'data/disclaimer.txt' } else { Join-Path $repo 'README.md' }
if (Test-Path $proof) {
    Pass "$what is back"
} else {
    Write-Host "$what IS NOT RESTORED. Look for *$SUFFIX under $repo" -ForegroundColor Red
    exit 2
}
$stragglers = @(Get-ChildItem -LiteralPath $repo -Force -EA SilentlyContinue | Where-Object Name -like "*$SUFFIX")
if ($stragglers.Count) {
    Write-Host "STILL HIDDEN: $($stragglers.Name -join ', ')" -ForegroundColor Red
    exit 2
}
if ($mode -eq 'one' -and (Test-Path $hidden)) {
    Write-Host "AND $hidden STILL EXISTS" -ForegroundColor Red
    exit 2
}

if ($script:failed) { Write-Host "clean-machine check FAILED" -ForegroundColor Red; exit 1 }
Write-Host "clean-machine check passed" -ForegroundColor Green
exit 0
