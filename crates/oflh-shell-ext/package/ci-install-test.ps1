<#
  CI check on a Windows runner (elevated): install the NSIS setup silently with
  the context menu option on (the default), check the classic registry verbs
  and the Windows 11 identity package, then uninstall and check both are gone.
#>
param([Parameter(Mandatory)] [string] $Setup)
$ErrorActionPreference = 'Stop'
$name = 'KarimZouine.OFLHDesktop.ContextMenu'
$verb = 'oflh-desktop.inspect'
$classes = @('*', 'Directory', 'Directory\Background', 'Drive')
$installDir = Join-Path $env:LOCALAPPDATA 'OFLH Desktop'
$failures = @()

function Wait-Installer([string] $Path, [string[]] $Arguments) {
  Start-Process -FilePath $Path -ArgumentList $Arguments -Wait
  # The uninstaller copies itself to %TEMP% and returns early.
  for ($i = 0; $i -lt 60 -and (Get-Process -Name 'Au_', 'un_*' -ErrorAction SilentlyContinue); $i++) { Start-Sleep 1 }
  Start-Sleep 2
}

Write-Host "Installing $Setup silently"
Wait-Installer $Setup @('/S')
foreach ($class in $classes) {
  $key = "Registry::HKEY_CURRENT_USER\Software\Classes\$class\shell\$verb\command"
  if (-not (Test-Path -LiteralPath $key)) { $failures += "classic verb missing: $class" }
}
foreach ($file in 'oflh_shell_ext.dll', 'oflh-context-menu.msix', 'register.ps1') {
  if (-not (Test-Path (Join-Path $installDir "shell-ext\$file"))) { $failures += "not installed: shell-ext\$file" }
}

$package = Get-AppxPackage -AllUsers -Name $name
if (-not $package) {
  $failures += 'identity package not registered'
  Get-Content (Join-Path $env:TEMP 'oflh-context-menu.log') -ErrorAction SilentlyContinue | Write-Host
  Get-WinEvent -LogName 'Microsoft-Windows-AppXDeploymentServer/Operational' -MaxEvents 15 -ErrorAction SilentlyContinue |
    Where-Object LevelDisplayName -in 'Error', 'Warning' | ForEach-Object { Write-Host "AppX: $($_.Message)" }
} else {
  $package | Format-List Name, PackageFullName, InstallLocation, IsDevelopmentMode, SignatureKind, Status | Out-String | Write-Host
  if ($package.InstallLocation.TrimEnd('\') -ne $installDir.TrimEnd('\')) { $failures += "external location is $($package.InstallLocation)" }
  $manifest = (Get-AppxPackageManifest -Package $package.PackageFullName).Package
  $menus = $manifest.Applications.Application.Extensions.Extension | Where-Object Category -eq 'windows.fileExplorerContextMenus'
  if (-not $menus) { $failures += 'manifest has no windows.fileExplorerContextMenus' }
  $types = @($menus.FileExplorerContextMenus.ItemType.Type)
  Write-Host "Context menu item types: $($types -join ', ')"
  # Activate the handler through the packaged COM registration, the way
  # Explorer does; this loads oflh_shell_ext.dll in the package's surrogate.
  try {
    $object = [Activator]::CreateInstance([Type]::GetTypeFromCLSID([guid]'6F1A0C4E-3B7D-4E59-9C2A-0F1E5D8B7A31'))
    Write-Host "Handler activated: $($object.GetType().FullName)"
    [void][Runtime.InteropServices.Marshal]::ReleaseComObject($object)
  } catch {
    $failures += "handler COM activation failed: $($_.Exception.Message)"
  }
}

Write-Host 'Uninstalling silently'
Wait-Installer (Join-Path $installDir 'uninstall.exe') @('/S')
foreach ($class in $classes) {
  if (Test-Path -LiteralPath "Registry::HKEY_CURRENT_USER\Software\Classes\$class\shell\$verb") { $failures += "classic verb left after uninstall: $class" }
}
if (Get-AppxPackage -AllUsers -Name $name) { $failures += 'identity package left after uninstall' }

if ($failures) {
  $failures | ForEach-Object { Write-Host "::error::$_" }
  exit 1
}
Write-Host 'Classic and Windows 11 context menus installed and removed cleanly.'
