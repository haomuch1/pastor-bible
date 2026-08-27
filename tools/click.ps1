# Click the app window at a point given as a fraction of it, and screenshot.
#
# tools/shot.ps1 does this too, but its click is a LEFTDOWN and a LEFTUP with no
# gap between them. WebView2 drops that press often enough to be useless for
# driving the app: on 2026-08-27 a run of footer clicks all missed while the
# same coordinates hit when the button was held for eighty milliseconds. So this
# holds the button, and waits after raising the window before it moves the
# cursor at all.
#
#   powershell -File tools/click.ps1 -X 0.044 -Y 0.958
#   powershell -File tools/click.ps1 -X 0.6 -Y 0.155 -Type "a question" -Keys "^{ENTER}"
#   powershell -File tools/click.ps1 -Out docs/screenshots/x.png          (no click)

param(
  [double]$X = -1,
  [double]$Y = -1,
  [string]$Out = "",
  [string]$Type = "",
  [string]$Keys = "",
  [int]$Wait = 1200,
  [int]$Hold = 90,
  [switch]$Front,
  [string]$Process = "pastor-bible"
)

Add-Type -AssemblyName System.Windows.Forms
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class Clk {
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int c);
  [DllImport("user32.dll")] public static extern bool MoveWindow(IntPtr h, int x, int y, int w, int t, bool r);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint x, uint y, uint d, int e);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
}
"@

$p = Get-Process -Name $Process -ErrorAction SilentlyContinue |
  Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
if (-not $p) { throw "no window for '$Process'" }
$h = $p.MainWindowHandle

[void][Clk]::ShowWindow($h, 9)
[void][Clk]::SetForegroundWindow($h)
Start-Sleep -Milliseconds 400
if ($Front) {
  [void][Clk]::MoveWindow($h, 60, 40, 1440, 960, $true)
  Start-Sleep -Milliseconds 500
  [void][Clk]::SetForegroundWindow($h)
  Start-Sleep -Milliseconds 300
}

$r = New-Object Clk+RECT
[void][Clk]::GetWindowRect($h, [ref]$r)

if ($X -ge 0 -and $Y -ge 0) {
  $px = [int]($r.Left + ($r.Right - $r.Left) * $X)
  $py = [int]($r.Top + ($r.Bottom - $r.Top) * $Y)
  [void][Clk]::SetCursorPos($px, $py)
  Start-Sleep -Milliseconds 250
  [Clk]::mouse_event(0x0002, 0, 0, 0, 0)
  Start-Sleep -Milliseconds $Hold
  [Clk]::mouse_event(0x0004, 0, 0, 0, 0)
  Start-Sleep -Milliseconds $Wait
}

if ($Type -ne "") {
  [System.Windows.Forms.SendKeys]::SendWait($Type)
  Start-Sleep -Milliseconds 400
}
if ($Keys -ne "") {
  [System.Windows.Forms.SendKeys]::SendWait($Keys)
  Start-Sleep -Milliseconds $Wait
}

if ($Out -ne "") {
  & "$PSScriptRoot\shot.ps1" -Out $Out -Process $Process
}
