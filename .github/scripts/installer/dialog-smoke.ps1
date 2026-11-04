# Build a small NSIS fixture; FlaUI/NUnit owns all UI automation and assertions.
param([Parameter(Mandatory=$true)][string]$Compiler)
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path "$PSScriptRoot/../../..").Path
$env:OFLH_DIALOG_SMOKE_EXE = Join-Path $env:RUNNER_TEMP 'oflh-dialog-smoke.exe'
$env:OFLH_DIALOG_SMOKE_RESULT = Join-Path $env:RUNNER_TEMP 'oflh-dialog-result.txt'
$env:OFLH_INSTALLER_REPORT = Join-Path $root 'installer-e2e-report'
& $Compiler "/DOUTPUT_EXE=$env:OFLH_DIALOG_SMOKE_EXE" "/DRESULT_OUTPUT=$env:OFLH_DIALOG_SMOKE_RESULT" "/DHOOKS=$root/crates/oflh-desktop/windows/installer-hooks.nsh" "$root/crates/oflh-desktop/tests/windows-installer/fixtures/dialog-smoke.nsi"
if ($LASTEXITCODE -ne 0) { throw 'NSIS fixture compilation failed' }
dotnet restore "$root/crates/oflh-desktop/tests/windows-installer/Installer.Tests.csproj" --locked-mode
if ($LASTEXITCODE -ne 0) { throw 'Test dependency restore failed' }
dotnet test "$root/crates/oflh-desktop/tests/windows-installer/Installer.Tests.csproj" --no-restore --filter 'TestCategory=DialogSmoke' --logger 'trx;LogFileName=dialog-smoke.trx' --results-directory $env:OFLH_INSTALLER_REPORT
if ($LASTEXITCODE -ne 0) { throw 'FlaUI native dialog test failed' }
