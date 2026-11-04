# Windows installer UI tests

This suite uses [FlaUI.UIA3](https://github.com/FlaUI/FlaUI) (MIT) and NUnit.
It lives with the desktop crate's other integration tests. It is not linked or
bundled into the application. GitHub Actions supplies an interactive Windows
runner and retains screenshots and TRX results for review.

`DialogSmoke` exercises the actual NSIS dialog macro at two owner positions,
checks that the dialog stays over and follows its owner, and presses Enter to
verify the default action is Cancel. `Installer` exercises the real built NSIS
package: silent installation, application startup, cancellation, confirmed
reinstallation, and silent uninstallation. All interaction and capture use
FlaUI; no custom Win32 automation driver is maintained.

From the repository root on Windows, restore the pinned dependencies:

```powershell
dotnet restore crates/oflh-desktop/tests/windows-installer/Installer.Tests.csproj --locked-mode
```

Run the full installer test against a built package:

```powershell
$env:OFLH_INSTALLER_EXE = (Resolve-Path 'target/release/bundle/nsis/*-setup.exe').Path
$env:OFLH_INSTALLER_REPORT = Join-Path $PWD 'installer-e2e-report'
dotnet test crates/oflh-desktop/tests/windows-installer/Installer.Tests.csproj --no-restore --filter 'TestCategory=Installer' --logger trx --results-directory installer-e2e-report
```

The CI smoke-test script builds the small NSIS fixture and runs `DialogSmoke`.
It requires a standard NSIS installation and the .NET 8 SDK.

