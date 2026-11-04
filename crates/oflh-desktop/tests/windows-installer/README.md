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

The CI smoke-test step compiles the small NSIS fixture and runs `DialogSmoke`.
It requires a standard NSIS installation and the .NET 8 SDK.


## Official references

- [FlaUI project and documentation](https://github.com/FlaUI/FlaUI)
- [Microsoft UI Automation](https://learn.microsoft.com/en-us/windows/win32/winauto/entry-uiauto-win32)
- [NUnit documentation](https://docs.nunit.org/articles/nunit/intro.html)
- [NSIS Modern UI 2](https://nsis.sourceforge.io/Docs/Modern%20UI%202/Readme.html)
- [Desktop development guide](../../../../docs/development.md#windows-installer-ui-automation)
- [Repository documentation index](../../../../llms.txt)

## Installer presentation

`../../windows/installer.nsi` is the Tauri CLI 2.12.0 NSIS template with small
presentation changes: Segoe UI, padded information panels, and page callbacks.
Its original MIT license is retained in `../../windows/TAURI_TEMPLATE_LICENSE`.
Compare the upstream template when upgrading the pinned Tauri CLI; the Rust
installer tests enforce that version relationship.

Branding words use native font controls so Windows renders them at display DPI.
Only the logo artwork is rasterized into the required high-resolution BMPs.
FlaUI checks accessible branding text, description margins/text fit, and saves
wizard-page screenshots for visual review. Generated screenshots and test/build
output are ignored by Git.
