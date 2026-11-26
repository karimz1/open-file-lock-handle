# Windows installer tests

.NET 10 test solution for the NSIS installer, built on open-source tools:
[NUnit](https://nunit.org/) (MIT) for fixtures and assertions and
[FlaUI.UIA3](https://github.com/FlaUI/FlaUI) (MIT) for Windows UI Automation and
screenshots. Registry, file system and process checks use plain .NET APIs. The
suite is not shipped with the application.

```
Oflh.Installer.Tests.sln
├── src/Oflh.Installer.Testing.Core            product facts, install options, timeouts, placement maths
├── src/Oflh.Installer.Testing.Infrastructure  FlaUI wizard driver, registry/file probes, process runner
├── tests/Oflh.Installer.Tests                 NUnit fixtures
└── fixtures/dialog-smoke.nsi                  tiny NSIS script around the production prompt macro
```

`Directory.Build.props` enables nullable reference types, analyzers and
warnings as errors; `Directory.Packages.props` pins every package version
centrally and each project has a lock file.

What is covered, and nothing more:

| Category | Fixture | Checks |
| --- | --- | --- |
| `DialogSmoke` | `RunningAppPromptTests` | The running-app prompt sits over the installer, follows it when moved, and Enter chooses Cancel. |
| `Installer` | `SilentInstallTests` | Silent install creates the executable, Start menu shortcut, Apps & features entry and the context-menu entries for files, folders, folder background and drives; uninstall removes them. `/NOCONTEXTMENU` installs none. |
| `Installer` | `UninstallWizardTests` | The interactive uninstaller over a running app: the confirm page shows native OFLH branding and the OFLH branding line, the running-app prompt closes the app, and the app is removed. Saves the confirm page and prompt as PNGs. |
| `Installer` | `WizardReinstallTests` | The interactive wizard over a running app: every page shows native OFLH branding and the OFLH branding line (never NSIS's default), and keeps text off the window edge, Cancel leaves the app running, confirming closes it and reaches Finish. Each page is saved as a PNG. |

Restore and run from the repository root on Windows:

```powershell
dotnet restore crates/oflh-desktop/tests/installer/Oflh.Installer.Tests.sln --locked-mode
$env:OFLH_INSTALLER_EXE = (Resolve-Path 'target/release/bundle/nsis/*-setup.exe').Path
$env:OFLH_INSTALLER_REPORT = Join-Path $PWD 'installer-e2e-report'
dotnet test crates/oflh-desktop/tests/installer/Oflh.Installer.Tests.sln --no-restore --filter 'TestCategory=Installer' --logger trx --results-directory installer-e2e-report
```

CI uploads the screenshots and TRX files as the `installer-e2e-<arch>` and
`installer-dialog-flaui` artifacts.

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
