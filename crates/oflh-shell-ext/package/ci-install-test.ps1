<#
  CI check on a Windows runner (elevated). With the context menu option on:
  install silently, check the identity package and that the classic verbs it
  covers are gone (no duplicates), probe both menus in Explorer, update over
  the running install (handler DLL loaded), toggle the option off and on, then
  uninstall and check that nothing is left.
#>
param([Parameter(Mandatory)] [string] $Setup, [string] $ScreenshotDir)
$ErrorActionPreference = 'Stop'
$name = 'KarimZouine.OFLHDesktop.ContextMenu'
$verb = 'oflh-desktop.inspect'
$covered = @('*', 'Directory', 'Directory\Background')
$installDir = Join-Path $env:LOCALAPPDATA 'OFLH Desktop'
$dll = Join-Path $installDir 'shell-ext\oflh_shell_ext.dll'
$script:failures = @()
function Fail([string] $text) { Write-Host "FAIL: $text"; $script:failures += $text }
function Verb-Exists([string] $class) { Test-Path -LiteralPath "Registry::HKEY_CURRENT_USER\Software\Classes\$class\shell\$verb" }

function Run-Installer([string] $Path, [string[]] $Arguments) {
  $p = Start-Process -FilePath $Path -ArgumentList $Arguments -Wait -PassThru
  # The uninstaller copies itself to %TEMP% and returns early.
  for ($i = 0; $i -lt 60 -and (Get-Process -Name 'Au_', 'un_*' -ErrorAction SilentlyContinue); $i++) { Start-Sleep 1 }
  Start-Sleep 2
  if ($p.ExitCode -ne 0) { Fail "$(Split-Path -Leaf $Path) $Arguments exited with $($p.ExitCode)" }
}

function Show-Diagnostics {
  Get-Content (Join-Path $env:TEMP 'oflh-context-menu.log') -ErrorAction SilentlyContinue | Write-Host
  Get-WinEvent -LogName 'Microsoft-Windows-AppXDeploymentServer/Operational' -MaxEvents 15 -ErrorAction SilentlyContinue |
    Where-Object LevelDisplayName -in 'Error', 'Warning' | ForEach-Object { Write-Host "AppX: $($_.Message)" }
}

# Expect the package registered for $installDir and only the Drive verb.
$helper = Join-Path $installDir 'shell-ext\oflh-shell-reg.exe'
# Package queries through Windows PowerShell commands (allowed when scripts are
# blocked); the Appx module does not load reliably in PowerShell 7.
function Find-Package {
  & "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -Command "Get-AppxPackage -AllUsers -Name $name | ForEach-Object { `$_.PackageFullName + ' ' + `$_.SignatureKind + ' ' + `$_.Status }"
}

# Expect the package registered for $installDir and only the Drive verb.
function Assert-Modern([string] $Stage) {
  $package = Find-Package
  if (-not $package) { Fail "${Stage}: identity package not registered"; Show-Diagnostics; return }
  $status = & $helper status
  if ($LASTEXITCODE -ne 0) { Fail "${Stage}: package not recorded for $installDir (status $LASTEXITCODE: $status)" }
  foreach ($class in $covered) { if (Verb-Exists $class) { Fail "${Stage}: classic verb duplicates the package: $class" } }
  if (-not (Verb-Exists 'Drive')) { Fail "${Stage}: drive verb missing" }
  if (-not (Test-Path $dll)) { Fail "${Stage}: $dll missing" }
  Write-Host "${Stage}: package $package"
}

function Assert-Nothing([string] $Stage) {
  foreach ($class in $covered + 'Drive') { if (Verb-Exists $class) { Fail "${Stage}: classic verb left: $class" } }
  if (Find-Package) { Fail "${Stage}: identity package left" }
}

# Scripts blocked for Windows PowerShell, as on many machines: the installer
# must not need them. (This CI script runs in PowerShell 7, whose policy is
# separate, so it can still call its helper script.)
$ps51 = "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe"
function Set-Policy([string] $Policy) {
  foreach ($scope in 'LocalMachine', 'CurrentUser') {
    & $ps51 -NoProfile -Command "Set-ExecutionPolicy -ExecutionPolicy $Policy -Scope $scope -Force"
  }
}
$savedPolicy = @{}
foreach ($scope in 'LocalMachine', 'CurrentUser') { $savedPolicy[$scope] = (& $ps51 -NoProfile -Command "Get-ExecutionPolicy -Scope $scope").Trim() }
Set-Policy 'Restricted'
$effective = (@('LocalMachine', 'CurrentUser') | ForEach-Object { "$_=$((& $ps51 -NoProfile -Command "Get-ExecutionPolicy -Scope $_").Trim())" }) -join ', '
Write-Host "Windows PowerShell execution policies: $effective"
if ($effective -notmatch 'LocalMachine=Restricted' -or $effective -notmatch 'CurrentUser=Restricted') { Fail "could not set execution policy Restricted ($effective)" }

Write-Host "1. Fresh silent install: $Setup"
Run-Installer $Setup @('/S')
Assert-Modern 'install'
$package = Get-AppxPackage -AllUsers -Name $name -ErrorAction SilentlyContinue
if ($package) {
  $manifest = (Get-AppxPackageManifest -Package $package.PackageFullName).Package
  $menus = $manifest.Applications.Application.Extensions.Extension | Where-Object Category -eq 'windows.fileExplorerContextMenus'
  Write-Host "Context menu item types: $(@($menus.FileExplorerContextMenus.ItemType.Type) -join ', ')"
  try {
    $object = [Activator]::CreateInstance([Type]::GetTypeFromCLSID([guid]'6F1A0C4E-3B7D-4E59-9C2A-0F1E5D8B7A31'))
    Write-Host "Handler activated: $($object.GetType().FullName)"
    [void][Runtime.InteropServices.Marshal]::ReleaseComObject($object)
  } catch { Fail "handler COM activation failed: $($_.Exception.Message)" }
}

if ($ScreenshotDir) {
  Write-Host '2. Explorer menus'
  try {
    $menu = & (Join-Path $PSScriptRoot 'ci-menu-screenshot.ps1') -OutDir $ScreenshotDir
    $seen = @($menu.cases.Values | Where-Object { $_.menuItems -gt 0 }).Count
    # Some runners (ARM64) have no interactive Explorer to right-click in.
    if ($seen -eq 0) { Write-Host '::warning title=Explorer menus::no context menu could be opened on this runner; menus not checked' }
    else { foreach ($case in $menu.cases.Keys) {
      $c = $menu.cases[$case]; $name2 = $case
      if (-not $c.modern) { Fail "$($c.label) missing from the Windows 11 menu ($name2)" }
      if ($null -eq $c.classicCount) { Fail "could not read the classic menu ($name2)" }
      elseif ($c.classicCount -ne 1) { Fail "classic menu ($name2) lists $($c.classicCount) OFLH entries, expected 1" }
    } }
    if (-not $menu.launched) { Write-Host '::warning title=Inspect file::clicking the entry did not visibly start OFLH' }
  } catch { Write-Host "::warning title=Context menu probe::$($_.Exception.Message)" }
}

Write-Host '3. Update over the existing install'
$loaded = Get-Process dllhost -ErrorAction SilentlyContinue | Where-Object { try { $_.Modules.FileName -contains $dll } catch { $false } }
Write-Host "Handler loaded in dllhost: $([bool]$loaded)"
Run-Installer $Setup @('/S')
Assert-Modern 'update'

Write-Host '4. Reinstall with the option off'
Run-Installer $Setup @('/S', '/NOCONTEXTMENU')
Assert-Nothing 'option off'

Write-Host '5. Reinstall with the option on again'
# /NOCONTEXTMENU stored "off"; a silent install keeps the stored choice, so
# switch it back the way the welcome page would.
Set-ItemProperty 'HKCU:\Software\Karim Zouine\OFLH Desktop' -Name ExplorerContextMenu -Value 1 -ErrorAction SilentlyContinue
Run-Installer $Setup @('/S')
Assert-Modern 'option on'

Write-Host '6. Uninstall'
Run-Installer (Join-Path $installDir 'uninstall.exe') @('/S')
Assert-Nothing 'uninstall'
if (Test-Path 'HKLM:\SOFTWARE\KarimZouine\OFLH Desktop') { Fail 'registration state left after uninstall' }

# CurrentUser first: it overrides LocalMachine, which would warn otherwise.
foreach ($scope in 'CurrentUser', 'LocalMachine') { & $ps51 -NoProfile -Command "Set-ExecutionPolicy -ExecutionPolicy $($savedPolicy[$scope]) -Scope $scope -Force" }

if ($script:failures) {
  $script:failures | ForEach-Object { Write-Host "::error::$_" }
  exit 1
}
Write-Host 'Context menus installed, updated, toggled and removed cleanly with scripts blocked.'
