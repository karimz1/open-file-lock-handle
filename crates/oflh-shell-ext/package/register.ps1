<#
  Registers or removes the OFLH Desktop Windows 11 context menu identity
  package. The NSIS installer runs this from <install dir>\shell-ext\.

  Exit codes: 0 done, 2 skipped (not Windows 11 or package missing), 1 failed.
  Status: 0 registered for this folder, 3 registered for another folder, 2 none.
  A log is written to %TEMP%\oflh-context-menu.log.
#>
param([ValidateSet('Register', 'Unregister', 'Status')] [string] $Action = 'Status')
$ErrorActionPreference = 'Stop'
# Windows PowerShell 5.1 started from PowerShell 7 inherits a PSModulePath that
# breaks loading its own Appx module; use the system value.
$env:PSModulePath = [Environment]::GetEnvironmentVariable('PSModulePath', 'Machine')
Import-Module (Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\Modules\Appx\Appx.psd1')
$name = 'KarimZouine.OFLHDesktop.ContextMenu'
# InstallLocation of a package with external location is its WindowsApps
# folder, so remember the external location here (machine-wide, like the
# unsigned package itself; readable without admin rights).
$stateKey = 'HKLM:\SOFTWARE\KarimZouine\OFLH Desktop'
$stateValue = 'ContextMenuExternalLocation'
$log = Join-Path $env:TEMP 'oflh-context-menu.log'
function Log([string] $text) { Add-Content -Path $log -Value "$(Get-Date -Format s) [$Action] $text" }

function Get-Registered {
  $isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole(
    [Security.Principal.WindowsBuiltInRole]::Administrator)
  if ($isAdmin) { @(Get-AppxPackage -AllUsers -Name $name) } else { @(Get-AppxPackage -Name $name) }
}

try {
  $installDir = Split-Path -Parent $PSScriptRoot
  switch ($Action) {
    'Status' {
      # 0: registered for this install folder, 3: registered elsewhere, 2: not registered.
      $packages = Get-Registered
      if ($packages.Count -eq 0) { exit 2 }
      $packages | ForEach-Object { Write-Output "$($_.PackageFullName) $($_.InstallLocation)" }
      $location = (Get-ItemProperty -Path $stateKey -Name $stateValue -ErrorAction SilentlyContinue).$stateValue
      Write-Output "External location: $location"
      if ($location -and $location.TrimEnd('\') -eq $installDir.TrimEnd('\')) { exit 0 } else { exit 3 }
    }
    'Register' {
      if ([Environment]::OSVersion.Version.Build -lt 22000) { Log 'Not Windows 11; classic menu only.'; exit 2 }
      $msix = Join-Path $PSScriptRoot 'oflh-context-menu.msix'
      if (-not (Test-Path $msix)) { Log "No $msix"; exit 2 }
      foreach ($old in Get-Registered) {
        Log "Removing $($old.PackageFullName)"
        Remove-AppxPackage -Package $old.PackageFullName -AllUsers -ErrorAction SilentlyContinue
        Remove-AppxPackage -Package $old.PackageFullName -ErrorAction SilentlyContinue
      }
      # package.ps1 leaves this marker next to packages it did not sign.
      $signed = -not (Test-Path (Join-Path $PSScriptRoot 'unsigned'))
      if ($signed) {
        Add-AppxPackage -Path $msix -ExternalLocation $installDir
      } else {
        # Windows 11 accepts unsigned packages whose publisher carries the
        # unsigned-package OID, only from an elevated process.
        Add-AppxPackage -Path $msix -ExternalLocation $installDir -AllowUnsigned
      }
      New-Item -Path $stateKey -Force | Out-Null
      Set-ItemProperty -Path $stateKey -Name $stateValue -Value $installDir
      Log "Registered (signed: $signed) for $installDir"
      exit 0
    }
    'Unregister' {
      foreach ($old in Get-Registered) {
        Log "Removing $($old.PackageFullName)"
        try { Remove-AppxPackage -Package $old.PackageFullName -AllUsers } catch { Remove-AppxPackage -Package $old.PackageFullName }
      }
      Remove-Item -Path $stateKey -Recurse -ErrorAction SilentlyContinue
      $parent = Split-Path -Parent $stateKey
      if ((Test-Path $parent) -and -not (Get-ChildItem $parent)) { Remove-Item $parent -ErrorAction SilentlyContinue }
      exit 0
    }
  }
} catch {
  Log "Failed: $_"
  Write-Error $_
  exit 1
}
