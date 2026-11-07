# Drives the debug build of Oxicord for looking at screens without a mouse.
#
# Start once and leave it running:
#   powershell -File scripts/drive.ps1 -Dir <outdir> -KeepAlive -Steps "wait:60"
# Then steer it as often as needed, no restart:
#   powershell -File scripts/drive.ps1 -Dir <outdir> -Attach -Steps "cmd:goto 123/mute/join 456;wait:5;shot:call"
#
# Steps are separated by ; and are: wait:s  click:x,y  rclick:x,y  move:x,y  wheel:x,y,delta
# type:text  enter  shot:name  cmd:line/line  (cmd writes the debug control file, see
# crates/oxicord-ui/src/dev_control.rs: goto <channel>, join <channel>, leave, mute, deafen).
# Coordinates are window pixels. Screenshots are of the app window only.


param([string]$Dir, [string]$Steps, [switch]$Attach, [switch]$KeepAlive)
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System; using System.Runtime.InteropServices;
public class W { [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint f);
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
 [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
 public struct RECT { public int L, T, R, B; } }
"@
if ($Attach) { $p = Get-Process oxicord | Select-Object -First 1 } else { $root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path; $p = Start-Process (Join-Path $root "target\debug\oxicord.exe") -PassThru -WorkingDirectory $root -RedirectStandardError "$Dir\err.txt" -RedirectStandardOutput "$Dir\out.txt" }
try {
  foreach ($s in $Steps.Split(';')) {
    $k, $v = $s.Split(':', 2)
    $p.Refresh(); $h = $p.MainWindowHandle
    switch ($k) {
      'wait' { Start-Sleep -Seconds ([int]$v) }
      'click' {
        $x, $y = $v.Split(',')
        $l = [IntPtr](([int]$y -shl 16) -bor [int]$x)
        [W]::PostMessage($h, 0x200, [IntPtr]0, $l) | Out-Null; Start-Sleep -Milliseconds 150
        [W]::PostMessage($h, 0x201, [IntPtr]1, $l) | Out-Null; Start-Sleep -Milliseconds 80
        [W]::PostMessage($h, 0x202, [IntPtr]0, $l) | Out-Null
      }
      'move' {
        $x, $y = $v.Split(',')
        $l = [IntPtr](([int]$y -shl 16) -bor [int]$x)
        [W]::PostMessage($h, 0x200, [IntPtr]0, $l) | Out-Null
      }
      'rclick' {
        $x, $y = $v.Split(',')
        $l = [IntPtr](([int]$y -shl 16) -bor [int]$x)
        [W]::PostMessage($h, 0x200, [IntPtr]0, $l) | Out-Null; Start-Sleep -Milliseconds 150
        [W]::PostMessage($h, 0x204, [IntPtr]2, $l) | Out-Null; Start-Sleep -Milliseconds 80
        [W]::PostMessage($h, 0x205, [IntPtr]0, $l) | Out-Null
      }
      'cmd' { Set-Content -Path (Join-Path $env:TEMP 'oxicord-dev-cmd.txt') -Value ($v.Replace('/', "`n")) -Encoding ascii }
      'wheel' {
        $x, $y, $d = $v.Split(',')
        $r = New-Object W+RECT; [W]::GetWindowRect($h, [ref]$r) | Out-Null
        $sx = $r.L + [int]$x; $sy = $r.T + [int]$y
        $l = [IntPtr](([int64]$sy -shl 16) -bor ([int64]$sx -band 0xffff))
        $w = [IntPtr](([int64]([int]$d -band 0xffff)) -shl 16)
        [W]::PostMessage($h, 0x20A, $w, $l) | Out-Null
      }
      'type' { foreach ($ch in $v.ToCharArray()) { [W]::PostMessage($h, 0x102, [IntPtr][int]$ch, [IntPtr]1) | Out-Null; Start-Sleep -Milliseconds 40 } }
      'enter' { [W]::PostMessage($h, 0x100, [IntPtr]13, [IntPtr]1) | Out-Null; Start-Sleep -Milliseconds 60; [W]::PostMessage($h, 0x102, [IntPtr]13, [IntPtr]1) | Out-Null; Start-Sleep -Milliseconds 60; [W]::PostMessage($h, 0x101, [IntPtr]13, [IntPtr]1) | Out-Null }
      'shot' {
        $r = New-Object W+RECT; [W]::GetWindowRect($h, [ref]$r) | Out-Null
        $bmp = New-Object System.Drawing.Bitmap ($r.R - $r.L), ($r.B - $r.T)
        $g = [System.Drawing.Graphics]::FromImage($bmp); $dc = $g.GetHdc()
        [W]::PrintWindow($h, $dc, 2) | Out-Null; $g.ReleaseHdc($dc)
        $bmp.Save((Join-Path $Dir "$v.png"))
      }
    }
  }
} finally { if (!$Attach -and !$KeepAlive -and !$p.HasExited) { $p.Kill() } }
"done"

