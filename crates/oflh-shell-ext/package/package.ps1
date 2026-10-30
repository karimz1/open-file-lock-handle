<#
.SYNOPSIS
  Builds the Windows 11 context menu identity package for OFLH Desktop.

.DESCRIPTION
  Copies the handler DLL, the identity package (MSIX) and register.ps1 into
  -OutDir, which the NSIS installer bundles as <install dir>\shell-ext\.

  Without signing secrets the package is unsigned and keeps the publisher with
  Microsoft's unsigned-package OID; Windows 11 then registers it only with
  Add-AppxPackage -AllowUnsigned from an elevated process.

  With OFLH_MSIX_PFX_BASE64 (and optionally OFLH_MSIX_PFX_PASSWORD) set, the
  publisher becomes the certificate subject and the package is signed with
  SignTool. That is the path for a certificate that is trusted on end-user
  machines (for example Azure Artifact Signing / a code-signing CA).
#>
param(
  [Parameter(Mandatory)] [string] $Version,
  [Parameter(Mandatory)] [string] $DllPath,
  [Parameter(Mandatory)] [string] $OutDir
)
$ErrorActionPreference = 'Stop'

function Find-SdkTool([string] $Name) {
  $roots = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin" -Directory -ErrorAction SilentlyContinue |
    Where-Object { $_.Name -match '^10\.' } | Sort-Object { [version]$_.Name } -Descending
  foreach ($root in $roots) {
    $tool = Join-Path $root.FullName "x64\$Name"
    if (Test-Path $tool) { return $tool }
  }
  throw "$Name not found in the Windows SDK"
}

# MSIX versions are four numbers; drop any pre-release or build suffix.
$core = ($Version -split '[-+]')[0]
$parts = @($core -split '\.' | ForEach-Object { [int]$_ })
while ($parts.Count -lt 4) { $parts += 0 }
$msixVersion = ($parts[0..3] -join '.')

$here = $PSScriptRoot
$icons = Join-Path $here '..\..\oflh-desktop\icons'
$stage = Join-Path ([IO.Path]::GetTempPath()) "oflh-msix-$([guid]::NewGuid())"
New-Item -ItemType Directory -Force -Path "$stage\Assets", $OutDir | Out-Null

$manifest = (Get-Content -Raw -Encoding UTF8 (Join-Path $here 'AppxManifest.xml')).Replace('@VERSION@', $msixVersion)

$pfx = $null
if ($env:OFLH_MSIX_PFX_BASE64) {
  $pfx = Join-Path $stage '..\oflh-msix-signing.pfx'
  [IO.File]::WriteAllBytes($pfx, [Convert]::FromBase64String($env:OFLH_MSIX_PFX_BASE64))
  $cert = [Security.Cryptography.X509Certificates.X509Certificate2]::new($pfx, $env:OFLH_MSIX_PFX_PASSWORD)
  $manifest = $manifest -replace 'Publisher="[^"]*"', ('Publisher="' + [Security.SecurityElement]::Escape($cert.Subject) + '"')
  Write-Host "Signing the identity package as $($cert.Subject)"
} else {
  Write-Host 'No OFLH_MSIX_PFX_BASE64: building an unsigned identity package (unsigned-publisher OID).'
}
[IO.File]::WriteAllText("$stage\AppxManifest.xml", $manifest, [Text.UTF8Encoding]::new($false))
foreach ($asset in 'StoreLogo', 'Square150x150Logo', 'Square44x44Logo') {
  Copy-Item (Join-Path $icons '128x128.png') "$stage\Assets\$asset.png"
}

$msix = Join-Path $OutDir 'oflh-context-menu.msix'
& (Find-SdkTool 'makeappx.exe') pack /o /nv /d $stage /p $msix
if ($LASTEXITCODE -ne 0) { throw "makeappx failed with $LASTEXITCODE" }

if ($pfx) {
  $signArgs = @('sign', '/fd', 'SHA256', '/f', $pfx)
  if ($env:OFLH_MSIX_PFX_PASSWORD) { $signArgs += @('/p', $env:OFLH_MSIX_PFX_PASSWORD) }
  if ($env:OFLH_MSIX_TIMESTAMP_URL) { $signArgs += @('/tr', $env:OFLH_MSIX_TIMESTAMP_URL, '/td', 'SHA256') }
  & (Find-SdkTool 'signtool.exe') @signArgs $msix
  $code = $LASTEXITCODE
  Remove-Item -Force $pfx
  if ($code -ne 0) { throw "signtool failed with $code" }
}

Copy-Item $DllPath (Join-Path $OutDir 'oflh_shell_ext.dll')
Copy-Item (Join-Path $here 'register.ps1') (Join-Path $OutDir 'register.ps1')
Remove-Item -Recurse -Force $stage
Write-Host "Identity package $msixVersion written to $OutDir"
