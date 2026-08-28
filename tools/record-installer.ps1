# Record every screen an installer shows, in order.
#
# P7 found a five-screen upgrade with two questions about deleting the reader's
# data. "The upgrade test passed" said nothing about that, because it ran with
# /S and saw no screens at all. So this watches the installer's own windows and
# writes down what each one said.
param(
    [Parameter(Mandatory = $true)][string] $Installer,
    [Parameter(Mandatory = $true)][string] $OutDir,
    [int] $MaxSeconds = 300
)
$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force $OutDir | Out-Null

Add-Type @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public class W {
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumWindowsProc cb, IntPtr p);
  [DllImport("user32.dll")] public static extern bool EnumChildWindows(IntPtr h, EnumWindowsProc cb, IntPtr p);
  public delegate bool EnumWindowsProc(IntPtr h, IntPtr p);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassNameW(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  public static string Text(IntPtr h){ var s=new StringBuilder(2048); GetWindowTextW(h,s,2048); return s.ToString(); }
  public static string Cls(IntPtr h){ var s=new StringBuilder(256); GetClassNameW(h,s,256); return s.ToString(); }
}
"@

function Snapshot([int] $pid_) {
    $lines = New-Object System.Collections.Generic.List[string]
    $top = New-Object System.Collections.Generic.List[IntPtr]
    $cb = [W+EnumWindowsProc] {
        param($h, $p)
        $wpid = 0
        [void][W]::GetWindowThreadProcessId($h, [ref]$wpid)
        if ($wpid -eq $pid_ -and [W]::IsWindowVisible($h)) { $top.Add($h) }
        return $true
    }
    [void][W]::EnumWindows($cb, [IntPtr]::Zero)
    foreach ($h in $top) {
        $t = [W]::Text($h)
        if ($t) { $lines.Add("WINDOW: $t") }
        $ccb = [W+EnumWindowsProc] {
            param($c, $p)
            if ([W]::IsWindowVisible($c)) {
                $ct = [W]::Text($c)
                $cc = [W]::Cls($c)
                if ($ct -and $cc -notmatch 'SysTreeView|SysListView') { $lines.Add("  [$cc] $ct") }
            }
            return $true
        }
        [void][W]::EnumChildWindows($h, $ccb, [IntPtr]::Zero)
    }
    return ($lines -join "`n")
}

$log = Join-Path $OutDir 'screens.txt'
"SCREEN-BY-SCREEN RECORD" | Out-File $log -Encoding utf8
"installer: $Installer" | Out-File $log -Append -Encoding utf8
"started:   (clock not used; order is what matters)" | Out-File $log -Append -Encoding utf8
"" | Out-File $log -Append -Encoding utf8

$proc = Start-Process $Installer -PassThru
Write-Host "installer pid $($proc.Id)"
$seen = @()
$n = 0
$deadline = (Get-Date).AddSeconds($MaxSeconds)

while ((Get-Date) -lt $deadline -and -not $proc.HasExited) {
    Start-Sleep -Milliseconds 400
    $snap = Snapshot $proc.Id
    if (-not $snap) { continue }
    if ($seen -contains $snap) { continue }
    $seen += $snap
    $n++
    Write-Host "--- screen $n ---"
    Write-Host $snap
    "=== SCREEN $n ===" | Out-File $log -Append -Encoding utf8
    $snap | Out-File $log -Append -Encoding utf8
    "" | Out-File $log -Append -Encoding utf8
}

if (-not $proc.HasExited) {
    Write-Host "installer still open after $MaxSeconds s (waiting on a button?)"
} else {
    Write-Host "installer exited $($proc.ExitCode)"
}
"distinct screens: $n" | Out-File $log -Append -Encoding utf8
Write-Host "distinct screens: $n  -> $log"
