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
change before tagging. The commands below use `v0.1.0` as an example; substitute
the intended version consistently.

## Package locally

```sh
cargo build --release --locked --bin oflh
cargo xtask package --version v0.1.0 --os linux --arch amd64 --binary target/release/oflh
```

Use the appropriate OS/architecture and `.exe` suffix on Windows. `package`
copies an existing executable; it does not change its embedded version. Check
`oflh --version` before packaging.

To assemble a complete release, download all six artifacts from the same passing
CI revision into `dist`, then run:

```sh
mkdir -p bin
cargo xtask assemble --version v0.1.0 --output dist --formula bin/oflh.rb
(cd dist && sha256sum --check checksums.txt)
```

The assembler rejects missing, empty, or unexpected artifacts and writes SHA-256
checksums and a Homebrew formula. Assembly is local; publication requires the
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
3. Verify the repository visibility, README status, and release metadata.
4. Publish the reviewed draft in GitHub Releases.
5. Publishing triggers **Update Homebrew tap**, which immediately dispatches **Update oflh** in `homebrew-tap`.

The tap updater only follows stable **published** releases. It verifies the four Unix executables against the release checksums and generates
`Formula/oflh.rb` locally. It leaves other tools' formulae untouched. The next `brew update` makes the new version
available via `brew install karimz1/tap/oflh` on Linux or macOS, Intel or ARM.

## Desktop packages

The release workflow also calls `desktop.yml`, which builds and tests the desktop
service with Tauri on the same six native targets. It collects DEB/RPM packages
on Linux, DMGs on macOS, and NSIS installers on Windows. These jobs are separate
from the existing CLI artifacts and Homebrew formula generation.

The desktop version is inherited from `workspace.package.version`. Build with
`tauri build --no-bundle -- --locked`, then use `tauri bundle --no-binary-patching`
to package that executable without rebuilding or patching it. See the
[desktop build guide](desktop.md#packages) for commands. Signing/notarization is
not configured; unsigned installers require platform review before distribution.

`cargo xtask package-desktop` records the tested executable hash and collects
platform packages with checksummed manifests. `cargo xtask assemble-desktop`
requires all six native sets, matching release versions and checksums; it rejects
missing, altered and unexpected artifacts. Desktop outputs live in
`desktop-dist`, never in the CLI assembler's `dist` directory. Both artifact sets
are uploaded to the same draft only after all jobs pass. Existing published
releases remain protected from overwrite.

The added workflow is configuration, not evidence of six passing native jobs.
Review the actual CI runs and manually exercise native dialogs, drag-and-drop,
clipboard, file-manager actions and installers on each OS before publishing the
first desktop RC. No tag, push or publication is part of local development.
