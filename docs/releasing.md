# Releasing

The [Release workflow](../.github/workflows/release.yml) builds and tests the CLI
and Desktop on Linux, macOS, and Windows, for x86-64 and ARM64. It checks the
Homebrew formula, assembles downloads and checksums, and creates a GitHub draft.
It does not publish automatically or overwrite a published release.

For local builds, see [Development](development.md).

## Prepare a release

1. Set `workspace.package.version` in `Cargo.toml` and refresh `Cargo.lock` with
   `cargo check`. The tag must match this version, prefixed with `v`.
2. Run `cargo xtask check`, commit the changes, and review CI results.
3. Tag and push the reviewed commit. For example:

```sh
git tag v0.2.0-rc.1
git push origin v0.2.0-rc.1
```

Use the intended version throughout. A version with a suffix such as `-rc.1`
creates a prerelease. Release executables and installer metadata use the tag
without `v`; local builds display **development**.

## Review the draft

Check that all native jobs passed for the tagged commit and the expected assets
are present. Write release notes describing changes users will notice, then
publish the draft in GitHub Releases.

Browser tests use sample results; they do not exercise native dialogs or every
installer interaction. Check affected native behavior when a release changes it.

To retry an existing tag:

```sh
gh workflow run release.yml --ref v0.2.0-rc.1 -f tag=v0.2.0-rc.1
```

Do not move a published tag.

## Desktop packages

CI builds the executable, tests it, and bundles that same binary using
`tauri bundle --no-binary-patching`. Packages are DEB/RPM on Linux, DMG on macOS,
and NSIS installers on Windows. The Windows download is an installer, not a
portable executable.

Rust `xtask` collects the six native artifact sets and validates their receipts,
versions, and hashes. `assemble-release` rejects missing or unexpected artifacts
and writes individual downloads, one `checksums.txt`, and the updater manifest.
Packaging commands and their arguments live in the [Desktop workflow](../.github/workflows/desktop.yml)
and [Release workflow](../.github/workflows/release.yml).

## Homebrew

Publishing triggers the tap update. Stable releases update `oflh-cli` and
`oflh-desktop`; prereleases update `oflh-cli-rc` and `oflh-desktop-rc`.
Downloads are verified against release checksums. Both channels install the same
command and app, so choose one. Scheduled updates follow stable releases only.

## Desktop updates and signing

Windows and macOS Desktop installs read `latest.json` from the latest stable
GitHub release. Linux DEB/RPM users update through their package manager or the
release page.

Updater signatures require a Tauri keypair. Keep the public key in
`tauri.conf.json` and the private key in the `TAURI_SIGNING_PRIVATE_KEY` repository
secret. Set `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` if the key has a password. Back up
the private key: existing installs need it to verify future updates.
Development CI builds without the secret use a temporary key and must not be
published as updates for existing installs. Tagged release builds require the
real signing secret and fail if it is missing. The Release workflow passes the
signing secrets explicitly to the reusable Desktop workflow.

Updater signing does not replace macOS notarization or Windows code signing.
These are not configured, so installers still show OS warnings.
