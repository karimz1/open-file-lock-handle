# Exercises the built NSIS installer in the runner's interactive desktop.
# No production application files or personal data are used.
param([Parameter(Mandatory=$true)][string]$Installer,
      [Parameter(Mandatory=$true)][string]$ReportDirectory)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class InstallerUI {
    public delegate bool EnumProc(IntPtr hwnd, IntPtr data);
    [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left, Top, Right, Bottom; }
    [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc callback, IntPtr data);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
    [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr hwnd);
    [DllImport("user32.dll")] public static extern IntPtr GetWindow(IntPtr hwnd, uint command);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hwnd, out Rect rect);
    [DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr hwnd, int id);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr hwnd, uint message, IntPtr wparam, IntPtr lparam);
    [DllImport("user32.dll", EntryPoint="GetWindowLongW")] public static extern int GetWindowStyle(IntPtr hwnd, int index);
    [DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr hwnd, uint message, IntPtr wparam, IntPtr lparam);
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr hwnd, IntPtr after, int x, int y, int width, int height, uint flags);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hwnd);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetWindowText(IntPtr hwnd, StringBuilder text, int size);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetClassName(IntPtr hwnd, StringBuilder text, int size);
    public static IntPtr[] Windows(int process) {
        var found = new List<IntPtr>();
        EnumWindows((hwnd, data) => { uint pid; GetWindowThreadProcessId(hwnd, out pid);
            if (pid == process && IsWindowVisible(hwnd)) found.Add(hwnd); return true; }, IntPtr.Zero);
        return found.ToArray();
    }
    public static string Text(IntPtr hwnd) { var text = new StringBuilder(1024); GetWindowText(hwnd, text, text.Capacity); return text.ToString(); }
    public static string Class(IntPtr hwnd) { var text = new StringBuilder(256); GetClassName(hwnd, text, text.Capacity); return text.ToString(); }
    public static Rect Bounds(IntPtr hwnd) { Rect rect; if (!GetWindowRect(hwnd, out rect)) throw new Exception("Window rectangle unavailable"); return rect; }
}
'@
function Wait-For([scriptblock]$Condition, [string]$Description, [int]$Seconds = 45) {
    $deadline = [DateTime]::UtcNow.AddSeconds($Seconds)
    do {
        $value = & $Condition
        if ($value) { return $value }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Timed out: $Description"
}
function Click-Control([IntPtr]$Window, [int]$Id) {
    $button = [InstallerUI]::GetDlgItem($Window, $Id)
    if ($button -eq [IntPtr]::Zero) { throw "Missing dialog button $Id" }
    # Asynchronous click: a synchronous BM_CLICK can block while a modal dialog runs.
    [void][InstallerUI]::PostMessage($Window, 0x0111, [IntPtr]$Id, [IntPtr]::Zero)
}
function Save-Window([IntPtr]$Window, [string]$Name) {
    $rect = [InstallerUI]::Bounds($Window)
    $image = [Drawing.Bitmap]::new($rect.Right - $rect.Left, $rect.Bottom - $rect.Top)
    $graphics = [Drawing.Graphics]::FromImage($image)
    try {
        $graphics.CopyFromScreen($rect.Left, $rect.Top, 0, 0, $image.Size)
        $image.Save((Join-Path $ReportDirectory "$Name.png"))
    } finally { $graphics.Dispose(); $image.Dispose() }
}
function Open-RunningPrompt {
    $setup = Start-Process -FilePath $Installer -ArgumentList @('/NOCONTEXTMENU', "/D=$installDirectory") -PassThru
    $script:setups += $setup
    $main = Wait-For {
        [InstallerUI]::Windows($setup.Id) | Where-Object {
            [InstallerUI]::GetWindow($_, 4) -eq [IntPtr]::Zero -and [InstallerUI]::Class($_) -eq '#32770'
        } | Select-Object -First 1
    } 'installer main window'
    # A screen-centered prompt fails this check: deliberately move its owner.
    [void][InstallerUI]::SetWindowPos($main, [IntPtr]::Zero, 70, 60, 0, 0, 0x0005)
    [void][InstallerUI]::SetForegroundWindow($main)
    Save-Window $main 'welcome'
    $deadline = [DateTime]::UtcNow.AddSeconds(45)
    $pageIndex = 0
    do {
        $prompt = [InstallerUI]::Windows($setup.Id) | Where-Object {
            $_ -ne $main -and [InstallerUI]::Class($_) -eq '#32770'
        } | Select-Object -First 1
        if ($prompt) {
            if ([InstallerUI]::GetWindow($prompt, 4) -ne $main) { throw 'Running-app dialog is not owned by the installer' }
            $ownerRect = [InstallerUI]::Bounds($main)
            $promptRect = [InstallerUI]::Bounds($prompt)
            $dx = [Math]::Abs(($ownerRect.Left + $ownerRect.Right - $promptRect.Left - $promptRect.Right) / 2)
            $dy = [Math]::Abs(($ownerRect.Top + $ownerRect.Bottom - $promptRect.Top - $promptRect.Bottom) / 2)
            if ($dx -gt 24 -or $dy -gt 24) { throw "Running-app dialog is displaced from its owner ($dx, $dy)" }
            $defaultButton = [InstallerUI]::SendMessage($prompt, 0x0400, [IntPtr]::Zero, [IntPtr]::Zero).ToInt64() -band 0xffff
            $cancelButton = [InstallerUI]::GetDlgItem($prompt, 2)
            $cancelStyle = [InstallerUI]::GetWindowStyle($cancelButton, -16) -band 0xf
            if ($defaultButton -ne 2 -and $cancelStyle -ne 1) { throw 'Running-app dialog does not default to Cancel' }
            Save-Window $main 'installer-behind-prompt'
            Save-Window $prompt 'running-app-prompt'
            return @{ Process = $setup; Main = $main; Prompt = $prompt }
        }
        if ($setup.HasExited) { throw "Installer exited before presenting the running-app dialog ($($setup.ExitCode))" }
        # Welcome -> maintenance (reinstall selected by default) -> directory -> install.
        if ($pageIndex -lt 6) { Save-Window $main "wizard-page-$pageIndex"; $pageIndex++ }
        Click-Control $main 1
        Start-Sleep -Milliseconds 500
    } while ([DateTime]::UtcNow -lt $deadline)
    throw 'Installer never showed the running-app prompt'
}
$Installer = (Resolve-Path $Installer).Path
[void](New-Item -ItemType Directory -Force $ReportDirectory)
$ReportDirectory = (Resolve-Path $ReportDirectory).Path
$installDirectory = Join-Path $env:RUNNER_TEMP 'oflh-installer-e2e'
$setups = @()
$app = $null
try {
    $initial = Start-Process -FilePath $Installer -ArgumentList @('/S', '/NOCONTEXTMENU', "/D=$installDirectory") -PassThru
    if (!$initial.WaitForExit(120000)) { $initial.Kill(); throw 'Silent installation timed out' }
    if ($initial.ExitCode -ne 0) { throw "Initial installation failed ($($initial.ExitCode))" }
    $installed = Join-Path $installDirectory 'oflh-desktop.exe'
    if (!(Test-Path $installed)) { throw 'Installer did not create the desktop executable' }
    $app = Start-Process -FilePath $installed -PassThru
    [void](Wait-For { [InstallerUI]::Windows($app.Id) | Select-Object -First 1 } 'installed app window')
    $cancel = Open-RunningPrompt
    Click-Control $cancel.Prompt 2
    Start-Sleep -Milliseconds 500
    if ($app.HasExited) { throw 'Cancel stopped the running application' }
    if (!$cancel.Process.HasExited) { $cancel.Process.Kill(); $cancel.Process.WaitForExit() }
    $accept = Open-RunningPrompt
    Click-Control $accept.Prompt 1
    [void](Wait-For { $app.Refresh(); $app.HasExited } 'confirmed close stops the installed app')
    # On the finish page the executable is present again after reinstall.
    [void](Wait-For {
        (Test-Path $installed) -and
        ([InstallerUI]::Text([InstallerUI]::GetDlgItem($accept.Main, 1)) -match 'Finish|Fertig|完成')
    } 'reinstallation reaches the finish page' 120)
    Save-Window $accept.Main 'reinstalled'
    if (!$accept.Process.HasExited) { $accept.Process.Kill(); $accept.Process.WaitForExit() }
    'Silent install, installed app startup, owned/centered prompt, default Cancel, cancellation and confirmed shutdown: PASS' |
        Set-Content (Join-Path $ReportDirectory 'results.txt')
} finally {
    foreach ($setup in $setups) { if (!$setup.HasExited) { $setup.Kill(); $setup.WaitForExit() } }
    if ($app -and !$app.HasExited) { $app.Kill(); $app.WaitForExit() }
    $uninstaller = Join-Path $installDirectory 'uninstall.exe'
    if (Test-Path $uninstaller) {
        $cleanup = Start-Process -FilePath $uninstaller -ArgumentList @('/S', "_?=$installDirectory") -PassThru
        if (!$cleanup.WaitForExit(60000)) { $cleanup.Kill() }
    }
    Remove-Item -Recurse -Force $installDirectory -ErrorAction SilentlyContinue
}
