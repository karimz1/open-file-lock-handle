<#
  CI probe (non-blocking): with OFLH installed, right-click a test file in File
  Explorer on the runner's interactive desktop, screenshot the Windows 11
  top-level menu and the classic "Show more options" menu, and report whether
  the OFLH entry appears in each, for a file, a folder and the folder
  background. The classic menu is opened with Shift+right-click.
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
  [DllImport("user32.dll")] public static extern IntPtr FindWindow(string c, string n);
  [DllImport("user32.dll")] static extern IntPtr SendMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
  [DllImport("user32.dll")] static extern int GetMenuItemCount(IntPtr m);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetMenuString(IntPtr m, uint id, System.Text.StringBuilder s, int max, uint flags);
  // Item labels of the open classic popup menu (#32768), via MN_GETHMENU.
  public static string[] ClassicMenu() {
    IntPtr w = FindWindow("#32768", null);
    if (w == IntPtr.Zero) return null;
    IntPtr m = SendMessage(w, 0x01E1, IntPtr.Zero, IntPtr.Zero);
    if (m == IntPtr.Zero) return null;
    int n = GetMenuItemCount(m);
    var names = new System.Collections.Generic.List<string>();
    for (int i = 0; i < n; i++) {
      var b = new System.Text.StringBuilder(256);
      GetMenuString(m, (uint)i, b, 256, 0x400);
      names.Add(b.ToString().Replace("&", ""));
    }
    return names.ToArray();
  }
  [DllImport("user32.dll")] static extern void keybd_event(byte k, byte s, uint f, UIntPtr e);
  public static void Shift(bool down) { keybd_event(0x10, 0, down ? 0u : 2u, UIntPtr.Zero); }
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

$dir = 'C:\oflh-menu-test'; New-Item -ItemType Directory -Force "$dir\oflh-folder" | Out-Null
Set-Content "$dir\oflh-test.txt" 'OFLH context menu probe'
Start-Process explorer.exe $dir
Start-Sleep 6
function Find-Item([string] $Name) {
  for ($i = 0; $i -lt 10; $i++) {
    $it = Find-Named "$Name*" | Where-Object { $_.Current.ControlType.ProgrammaticName -eq 'ControlType.ListItem' } | Select-Object -First 1
    if ($it) { return $it.Current.BoundingRectangle }
    Start-Sleep 2
  }
  Shot '0-explorer'; throw "$Name not found in Explorer"
}
$file = Find-Item 'oflh-test'
$folder = Find-Item 'oflh-folder'
# Empty space well below the two items, for the folder background.
$bg = [pscustomobject]@{ X = $file.X; Y = [Math]::Max($file.Bottom, $folder.Bottom) + 150; Width = $file.Width; Height = 1 }
function Center($r) { @([int]($r.X + $r.Width / 2), [int]($r.Y + $r.Height / 2)) }
function Open-Menu($r, [bool] $classic) {
  [Windows.Forms.SendKeys]::SendWait('{ESC}{ESC}'); Start-Sleep 1
  $x, $y = Center $r
  [OflhInput]::Click($x, $y, $false); Start-Sleep 1
  if ($classic) { [OflhInput]::Shift($true) }
  [OflhInput]::Click($x, $y, $true)
  if ($classic) { Start-Sleep -Milliseconds 300; [OflhInput]::Shift($false) }
  Start-Sleep 3
}
$cases = [ordered]@{}
$n = 1
# Not $t: PowerShell names are case-insensitive and $T is TreeScope.
foreach ($case in @(@{ name = 'file'; rect = $file; label = 'Inspect file' }, @{ name = 'folder'; rect = $folder; label = 'Inspect folder' }, @{ name = 'background'; rect = $bg; label = 'Inspect folder' })) {
  Open-Menu $case.rect $false
  Shot ("{0}-modern-{1}" -f $n++, $case.name)
  $names = Menu-Names
  Write-Host "Windows 11 menu ($($case.name)): $($names -join ' | ')"
  $modern = @($names | Where-Object { $_ -like "$($case.label)*" }).Count -gt 0
  Open-Menu $case.rect $true
  Shot ("{0}-classic-{1}" -f $n++, $case.name)
  # The classic menu is a Win32 popup (#32768); read its HMENU.
  $classic = [OflhInput]::ClassicMenu()
  Write-Host "Classic menu ($($case.name)): $($classic -join ' | ')"
  $count = if ($null -eq $classic) { $null } else { @($classic | Where-Object { $_ -like 'Inspect*' }).Count }
  $cases[$case.name] = [pscustomobject]@{ label = $case.label; modern = $modern; classicCount = $count; classicItems = $classic }
  Write-Host "$($case.name): '$($case.label)' in Windows 11 menu: $modern; OFLH entries in classic menu: $count"
}
[Windows.Forms.SendKeys]::SendWait('{ESC}{ESC}'); Start-Sleep 1

# Click the top-level file entry and check that it starts OFLH with the file.
$launched = $false
Open-Menu $file $false
$entry = Find-Named 'Inspect file' | Select-Object -First 1
if ($entry) {
  $x, $y = Center $entry.Current.BoundingRectangle
  [OflhInput]::Click($x, $y, $false)
  for ($i = 0; $i -lt 15 -and -not $launched; $i++) {
    Start-Sleep 1
    $app = Get-CimInstance Win32_Process -Filter "Name='oflh-desktop.exe'" | Where-Object CommandLine -like '*--inspect*oflh-test.txt*'
    if ($app) { $launched = $true; Write-Host "Started: $($app.CommandLine)" }
  }
  Start-Sleep 3
  Shot ("{0}-after-inspect-file" -f $n)
  Get-Process oflh-desktop -ErrorAction SilentlyContinue | Stop-Process -Force
}
Write-Host "Inspect file started OFLH: $launched"
$result = [pscustomobject]@{ cases = $cases; launched = $launched }
$result | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $OutDir 'result.json')
return $result
