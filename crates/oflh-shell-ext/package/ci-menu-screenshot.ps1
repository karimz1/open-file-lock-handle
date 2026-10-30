<#
  CI probe (non-blocking): with OFLH installed, right-click a test file in File
  Explorer on the runner's interactive desktop, screenshot the Windows 11
  top-level menu and the classic "Show more options" menu, and report whether
  "Inspect file" appears in each (UI Automation).
#>
param([Parameter(Mandatory)] [string] $OutDir)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes, System.Drawing, System.Windows.Forms
Add-Type @'
using System; using System.Runtime.InteropServices;
public static class OflhInput {
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, uint d, UIntPtr e);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  public static void Click(int x, int y, bool right) {
    SetCursorPos(x, y);
    if (right) { mouse_event(0x0008, 0, 0, 0, UIntPtr.Zero); mouse_event(0x0010, 0, 0, 0, UIntPtr.Zero); }
    else { mouse_event(0x0002, 0, 0, 0, UIntPtr.Zero); mouse_event(0x0004, 0, 0, 0, UIntPtr.Zero); }
  }
}
'@
New-Item -ItemType Directory -Force $OutDir | Out-Null
$A = [Windows.Automation.AutomationElement]
$T = [Windows.Automation.TreeScope]
function Shot([string] $Name) {
  $b = [Windows.Forms.Screen]::PrimaryScreen.Bounds
  $bmp = [Drawing.Bitmap]::new($b.Width, $b.Height)
  $g = [Drawing.Graphics]::FromImage($bmp); $g.CopyFromScreen($b.Location, [Drawing.Point]::Empty, $b.Size)
  $bmp.Save((Join-Path $OutDir "$Name.png")); $g.Dispose(); $bmp.Dispose()
}
function Find-Named([string] $Pattern) {
  $all = $A::RootElement.FindAll($T::Descendants, [Windows.Automation.Condition]::TrueCondition)
  @($all | Where-Object { $_.Current.Name -like $Pattern })
}
function Menu-Names {
  $cond = [Windows.Automation.PropertyCondition]::new($A::ControlTypeProperty, [Windows.Automation.ControlType]::MenuItem)
  @($A::RootElement.FindAll($T::Descendants, $cond) | ForEach-Object { $_.Current.Name })
}

$dir = 'C:\oflh-menu-test'; New-Item -ItemType Directory -Force $dir | Out-Null
Set-Content "$dir\oflh-test.txt" 'OFLH context menu probe'
Start-Process explorer.exe $dir
Start-Sleep 6
$item = $null
for ($i = 0; $i -lt 10 -and -not $item; $i++) {
  $item = Find-Named 'oflh-test*' | Where-Object { $_.Current.ControlType.ProgrammaticName -eq 'ControlType.ListItem' } | Select-Object -First 1
  if (-not $item) { Start-Sleep 2 }
}
if (-not $item) { Shot '0-explorer'; throw 'test file not found in Explorer' }
$r = $item.Current.BoundingRectangle
[OflhInput]::Click([int]($r.X + $r.Width / 2), [int]($r.Y + $r.Height / 2), $false)
Start-Sleep 1
[OflhInput]::Click([int]($r.X + $r.Width / 2), [int]($r.Y + $r.Height / 2), $true)
Start-Sleep 3
Shot '1-windows11-menu'
$modern = Menu-Names
Write-Host "Top-level menu items: $($modern -join ' | ')"
$result = [ordered]@{ modern = [bool]($modern -like '*Inspect file*'); classic = $false; modernItems = $modern }

$more = Find-Named 'Show more options*' | Select-Object -First 1
if ($more) {
  $m = $more.Current.BoundingRectangle
  [OflhInput]::Click([int]($m.X + $m.Width / 2), [int]($m.Y + $m.Height / 2), $false)
  Start-Sleep 3
  Shot '2-classic-menu'
  $classic = Menu-Names
  Write-Host "Classic menu items: $($classic -join ' | ')"
  $result.classic = [bool]($classic -like '*Inspect file*')
  $result.classicItems = $classic
}
[Windows.Forms.SendKeys]::SendWait('{ESC}{ESC}')
$result | ConvertTo-Json | Set-Content (Join-Path $OutDir 'result.json')
Write-Host "Inspect file in Windows 11 menu: $($result.modern); in classic menu: $($result.classic)"
if (-not $result.modern) { Write-Host '::warning title=Windows 11 menu::"Inspect file" was not found in the top-level context menu (see screenshots).' }
