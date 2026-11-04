param([Parameter(Mandatory=$true)][string]$Compiler)
$ErrorActionPreference = 'Stop'
# Use the same native window driver as the full installer E2E test.
$source = Get-Content -Raw "$PSScriptRoot/windows-e2e.ps1"
$driver = [regex]::Match($source, "(?s)Add-Type @'\r?\n(.*?)\r?\n'@").Groups[1].Value
if (!$driver) { throw 'Installer UI driver was not found' }
Add-Type $driver
$executable = Join-Path $env:RUNNER_TEMP 'oflh-dialog-smoke.exe'
$result = Join-Path $env:RUNNER_TEMP 'oflh-dialog-result.txt'
$hooks = (Resolve-Path "$PSScriptRoot/../../../crates/oflh-desktop/windows/installer-hooks.nsh").Path
& $Compiler "/DOUTPUT_EXE=$executable" "/DRESULT_OUTPUT=$result" "/DHOOKS=$hooks" "$PSScriptRoot/dialog-smoke.nsi"
if ($LASTEXITCODE -ne 0) { throw 'Native dialog fixture compilation failed' }
function Wait-Window([scriptblock]$Find, [string]$Description) {
    $until = [DateTime]::UtcNow.AddSeconds(30)
    do {
        $value = & $Find
        if ($value) { return $value }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $until)
    throw "Timed out waiting for $Description"
}
$process = Start-Process $executable -PassThru
try {
    $main = Wait-Window {
        [InstallerUI]::Windows($process.Id) | Where-Object { [InstallerUI]::GetWindow($_, 4) -eq [IntPtr]::Zero } | Select-Object -First 1
    } 'fixture owner window'
    [void][InstallerUI]::SetWindowPos($main, [IntPtr]::Zero, 70, 60, 0, 0, 5)
    [void][InstallerUI]::PostMessage($main, 0x0111, [IntPtr]1, [IntPtr]::Zero)
    $prompt = Wait-Window {
        if ($process.HasExited) { throw "Dialog fixture exited early ($($process.ExitCode))" }
        [InstallerUI]::Windows($process.Id) | Where-Object { [InstallerUI]::GetWindow($_, 4) -eq $main } | Select-Object -First 1
    } 'native task dialog'
    $ownerRect = [InstallerUI]::Bounds($main)
    $promptRect = [InstallerUI]::Bounds($prompt)
    $dx = [Math]::Abs(($ownerRect.Left + $ownerRect.Right - $promptRect.Left - $promptRect.Right) / 2)
    $dy = [Math]::Abs(($ownerRect.Top + $ownerRect.Bottom - $promptRect.Top - $promptRect.Bottom) / 2)
    if ($dx -gt 24 -or $dy -gt 24) { throw "Dialog is displaced from its owner ($dx, $dy)" }
    $defaultButton = [InstallerUI]::SendMessage($prompt, 0x0400, [IntPtr]::Zero, [IntPtr]::Zero).ToInt64() -band 0xffff
    $cancel = [InstallerUI]::GetDlgItem($prompt, 2)
    $style = [InstallerUI]::GetWindowStyle($cancel, -16) -band 0xf
    if ($defaultButton -ne 2 -and $style -ne 1) { throw 'Task dialog does not default to Cancel' }
    [void][InstallerUI]::PostMessage($prompt, 0x0111, [IntPtr]2, [IntPtr]::Zero)
    if (!$process.WaitForExit(10000)) { throw 'Cancel did not dismiss the dialog' }
    if ($process.ExitCode -ne 0) { throw "Dialog fixture failed ($($process.ExitCode))" }
    if ((Get-Content -Raw $result) -ne '2') { throw 'Dialog did not return Cancel to the installer' }
    Write-Host 'Native NSIS task dialog: owned, centered, default-cancel, and returns Cancel.'
} finally {
    if (!$process.HasExited) { $process.Kill(); $process.WaitForExit() }
}
