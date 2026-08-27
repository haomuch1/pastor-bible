# Capture the app window, and click and type into it.
#
# Claude Code cannot see the window, so screenshots are how P5's visual work is
# handed to Jared at all. This is the smallest thing that does it: find the
# window by process name, bring it to the front, copy its rectangle off the
# screen, and save a PNG. Clicks are given in fractions of the window so they do
# not depend on where the window happens to be.
#
#   powershell -File tools/shot.ps1 -Out docs/screenshots/main.png
#   powershell -File tools/shot.ps1 -Out x.png -ClickX 0.1 -ClickY 0.97
#   powershell -File tools/shot.ps1 -Out x.png -Type "a question" -Keys "^{ENTER}"

param(
  [string]$Out = "",
  [string]$Process = "pastor-bible",
  [double]$ClickX = -1,
  [double]$ClickY = -1,
  [string]$Type = "",
  [string]$Keys = "",
  [int]$Wait = 700,
  [switch]$Front,
  [switch]$NoPrintWindow
)

Add-Type -AssemblyName System.Drawing, System.Windows.Forms

Add-Type @"
using System;
using System.Runtime.InteropServices;
public class Win {
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int c);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint x, uint y, uint d, int e);
  [DllImport("user32.dll")] public static extern bool MoveWindow(IntPtr h, int x, int y, int w, int t, bool r);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint flags);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
}
"@

function Get-AppWindow {
  $p = Get-Process -Name $Process -ErrorAction SilentlyContinue |
       Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
  if (-not $p) { throw "no window found for process '$Process'" }
  return $p.MainWindowHandle
}

$h = Get-AppWindow
[void][Win]::ShowWindow($h, 9)          # restore if minimised
[void][Win]::SetForegroundWindow($h)
Start-Sleep -Milliseconds 250

# A fixed window size keeps every screenshot the same shape and makes the
# fractional click positions mean the same thing from one run to the next.
if ($Front) {
  [void][Win]::MoveWindow($h, 60, 40, 1440, 960, $true)
  Start-Sleep -Milliseconds 400
}

$r = New-Object Win+RECT
[void][Win]::GetWindowRect($h, [ref]$r)

if ($ClickX -ge 0 -and $ClickY -ge 0) {
  $x = [int]($r.Left + ($r.Right - $r.Left) * $ClickX)
  $y = [int]($r.Top + ($r.Bottom - $r.Top) * $ClickY)
  [void][Win]::SetCursorPos($x, $y)
  Start-Sleep -Milliseconds 120
  [Win]::mouse_event(0x0002, 0, 0, 0, 0)   # left down
  [Win]::mouse_event(0x0004, 0, 0, 0, 0)   # left up
  Start-Sleep -Milliseconds $Wait
}

if ($Type -ne "") {
  [System.Windows.Forms.SendKeys]::SendWait($Type)
  Start-Sleep -Milliseconds 250
}
if ($Keys -ne "") {
  [System.Windows.Forms.SendKeys]::SendWait($Keys)
  Start-Sleep -Milliseconds $Wait
}

if ($Out -ne "") {
  Start-Sleep -Milliseconds 200
  [void][Win]::GetWindowRect($h, [ref]$r)
  $w = $r.Right - $r.Left
  $t = $r.Bottom - $r.Top
  if ($w -le 0 -or $t -le 0) { throw "the window has no size" }
  $bmp = New-Object System.Drawing.Bitmap $w, $t
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  # WebView2 draws into a composition surface that a plain screen copy can miss,
  # so ask the window to render itself first; fall back to the screen if it will
  # not, which is what happens on window frames that refuse PrintWindow.
  $used = $false
  if (-not $NoPrintWindow) {
    $hdc = $g.GetHdc()
    $used = [Win]::PrintWindow($h, $hdc, 2)   # PW_RENDERFULLCONTENT
    $g.ReleaseHdc($hdc)
  }
  if (-not $used) { $g.CopyFromScreen($r.Left, $r.Top, 0, 0, $bmp.Size) }
  $dir = Split-Path -Parent $Out
  if ($dir -and -not (Test-Path $dir)) { New-Item -ItemType Directory -Force $dir | Out-Null }
  $bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
  $g.Dispose(); $bmp.Dispose()
  Write-Output ("saved {0}  {1}x{2}" -f $Out, $w, $t)
}
