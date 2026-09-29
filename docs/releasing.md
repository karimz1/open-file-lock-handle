# Releases and the Homebrew tap

The public release workflow creates **drafts only** for review before publishing.

## What CI tests

Every push to `main` and every pull request runs the following matrix:

| Runner | Target |
| --- | --- |
| Ubuntu 24.04 x86-64 | linux/amd64 |
| Ubuntu 24.04 ARM64 | linux/arm64 |
| macOS 15 Intel | darwin/amd64 |
| macOS 15 Apple Silicon | darwin/arm64 |
| Windows 2025 x86-64 | windows/amd64 |
| Windows 11 ARM64 | windows/arm64 |

The CLI workflow runs formatting, Clippy, unit/native integration tests, and
repeated real terminal tests on every target. It builds the release `oflh`
executable, checks its CLI, and packages it with Rust `xtask`. The separate Desktop
workflow tests the frontend and native backend before building `oflh-desktop`
and its installers on the same six targets. Release jobs consume both sets of
artifacts. See the [Desktop RC checklist](desktop-rc.md) for acceptance gates.

Tests and developer tools are separate executables and dev-dependencies. The
application needs no Go or Python runtime. Release builds use thin LTO, one
codegen unit, stripped symbols, and unwinding for RAII terminal cleanup.

## Prepare a version

Update `workspace.package.version` in `Cargo.toml` and refresh `Cargo.lock` with
`cargo check`. Use semantic versions: `0.1.0-rc.1` for a candidate or `0.1.0` for a
stable release. Record user-visible changes in the release notes, not in temporary
README status messages.

Run `cargo xtask check`, review all six native CI jobs, and commit the version
change before tagging. The commands below use `v0.1.0` as an example.
Substitute the intended version consistently.
the intended version consistently.

## Package locally

```sh
cargo build --release --locked --bin oflh
cargo xtask package --version v0.1.0 --os linux --arch amd64 --binary target/release/oflh
```

Use the appropriate OS/architecture and `.exe` suffix on Windows. `package`
copies an existing executable. It does not change its embedded version. Check
`oflh --version` before packaging.

To assemble a complete release, download all six artifacts from the same passing
CI revision into `dist`, then run:

```sh
mkdir -p bin
cargo xtask assemble --version v0.1.0 --output dist --formula bin/oflh-cli.rb
(cd dist && sha256sum --check checksums.txt)
```

The assembler rejects missing, empty, or unexpected artifacts and writes SHA-256
checksums and a Homebrew formula. Assembly is local. Publication requires the
release workflow below. Formula URLs refer to the corresponding GitHub release.

## Create a draft

From a reviewed commit:

```sh
git tag v0.1.0
git push origin v0.1.0
```

The Release workflow reruns the full matrix for that tag and installs/tests the generated Homebrew formula on Linux and macOS. If any target fails, no draft
is created. A passing run uploads six raw executables and `checksums.txt` to the draft release. Version strings omit the leading `v`.

To retry an existing tag, use the workflow UI and select **that tag** as the workflow
ref, or run:

```sh
gh workflow run release.yml --ref v0.1.0 -f tag=v0.1.0
```

The workflow verifies that the tag resolves to the workflow commit, refuses to overwrite
published releases, and allows updating an existing draft. Do not move a published tag.

## Before public distribution

1. Review all six green CI jobs and the draft artifacts.
2. Decide whether to add Developer ID signing/notarization for macOS. It is not configured.
3. Review the GitHub release title and generated notes. Use a title such as
	`OFLH v0.1.0-rc.1` and replace commit noise with a short user-facing summary.
4. Check the repository social preview in **Settings > General > Social preview**.
	Use an image that shows both the Desktop and terminal interfaces.
5. Verify repository visibility, README status, and release metadata.
6. Publish the reviewed draft in GitHub Releases.
7. Publishing triggers **Update Homebrew tap**, which immediately dispatches
	**Update oflh** in `homebrew-tap`.

The tap updater only follows stable **published** releases. It verifies every
download against the shared checksums and generates `Formula/oflh-cli.rb` plus
the macOS `Casks/oflh-desktop.rb` when Desktop packages are available. The next
`brew update` makes the CLI available with `brew install karimz1/tap/oflh-cli` on
Linux or macOS, Intel or ARM. `brew install karimz1/tap/oflh` remains an alias.

## Desktop packages

The release workflow also calls `desktop.yml`, which builds and tests the desktop
service with Tauri on the same six native targets. It collects DEB/RPM packages
on Linux, DMGs on macOS, and NSIS installers on Windows. These jobs are separate
from the existing CLI artifacts and Homebrew formula generation.

Both frontends use `oflh_core::VERSION`: release CI sets `OFLH_VERSION` to
the tag without `v`, and supplies the same version through a Tauri config override
for both build and bundle. Local builds display `development`. The workspace
package version is the internal Cargo version. Build with
`tauri build --no-bundle -- --locked`, then use `tauri bundle --no-binary-patching`
to package that executable without rebuilding or patching it. See the
[desktop build guide](desktop.md#packages) for commands. Signing/notarization is
not configured. Unsigned installers require platform review before distribution.

Public download names distinguish interfaces: `oflh-cli.linux.amd64`,
`oflh-cli.windows.arm64.exe`, `oflh-desktop.linux.amd64.rpm`,
`oflh-desktop.linux.amd64.deb`, `oflh-desktop.darwin.arm64.dmg`, and
`oflh-desktop.windows.arm64.exe`. The installed terminal command remains `oflh`.
Older releases retain their existing `oflh-linux-amd64` style names.

`cargo xtask package-desktop` records tested executable and installer hashes in
internal validation receipts. `assemble-desktop` requires all six native sets,
matching versions and checksums and rejects missing, altered or extra artifacts.
Receipts are uploaded separately for CI assembly. They are not public release assets.

`cargo xtask assemble-release --version TAG --cli-dir dist --desktop-dir desktop-dist --output release-dist`
validates both matrices, collects only the six CLI executables and eight native
installers, and writes **one `checksums.txt`** covering all 14 downloads. It refuses
to overwrite its output directory. The release workflow uploads these files
individually, without ZIP wrappers or manifests. Native package Actions artifacts
also use `archive: false`. Diagnostic reports and internal receipts remain grouped.

The tap updater accepts legacy CLI-only releases and combined CLI/Desktop
releases. It verifies every asset against the release checksums and generates
the `oflh-desktop` Cask from both macOS DMGs. RC users download installers
directly. Existing published releases remain protected from overwrite.

Installer metadata names Karim Zouine as publisher. Windows verified publisher
status and macOS notarization still require signing. Metadata alone does not remove
OS warnings. No signing credentials are configured by this change.

The added workflow is configuration, not evidence of six passing native jobs.
Review the actual CI runs and manually exercise native dialogs, drag-and-drop,
clipboard, file-manager actions and installers on each OS before publishing the
first desktop RC. No tag, push or publication is part of local development.
