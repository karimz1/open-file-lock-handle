# Releasing

The [Release workflow](../.github/workflows/release.yml) builds and tests the CLI
and Desktop on Linux, macOS, and Windows, for x86-64 and ARM64. It checks the
Homebrew formula, assembles downloads and checksums, and creates a GitHub draft.
It does not publish automatically or overwrite a published release.

For local builds, see [Development](development.md).

## CI artifact storage

Push and pull-request builds disable optional artifact uploads by default:
CLI downloads, macOS/Windows desktop downloads, desktop validation receipts,
and browser test reports. Linux desktop archives, DEBs, and RPMs still upload
because the installation and archive smoke-test jobs download those exact
packages. Every Actions artifact expires after one day, including release builds.
Builds, signing checks, and native/package tests still run.

For temporary downloads or browser test reports, manually run **CI** or
**Desktop** from GitHub Actions with `upload_artifacts` enabled. The default is
disabled; **Desktop**'s `administrator_only` mode does not produce packages.

The **Release** workflow explicitly enables the uploads it needs to assemble
the tested downloads. For downloads you want to keep, use an RC tag as described
below. Files copied to the GitHub release page stay there independently of
Actions artifact expiration. The workflow leaves the RC as a draft for review;
publish it as a prerelease when it is ready to share. If intermediate artifacts
have expired before a retry, rerun the entire Release workflow to rebuild them.

These settings apply to new uploads; previously uploaded artifacts keep their
existing expiry dates. See GitHub's documentation on
[removing workflow artifacts](https://docs.github.com/en/actions/how-tos/manage-workflow-runs/remove-workflow-artifacts)
to reclaim existing storage.

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
creates a prerelease and is explicitly excluded from GitHub's **Latest** release.
The latest stable release remains the default download and updater destination.
Release executables and installer metadata use the tag
without `v`; local builds display **development**.

## Review the draft

Check that all native jobs passed for the tagged commit and the expected assets
are present. Write release notes describing changes users will notice, then
publish the draft in GitHub Releases.

New drafts prepend a [download introduction](../.github/release-notes/downloads.md)
to GitHub's generated **What's Changed** section and full changelog. It links to
the website's latest stable downloads and documentation for newcomers, and
directs experienced users to this GitHub release's version-specific assets and
checksums, especially for release candidates. Retrying an
existing draft preserves edited release notes and reinforces its prerelease
classification for RC tags. Published releases cannot be modified by a retry.

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

Windows and macOS Desktop installs read the static manifest at
`https://oflh.karimzouine.com/api/latest.json`. Linux DEB/RPM users update through
their package manager or the release page.

The Release workflow automatically commits each assembled manifest, including
RCs, to `karimz1/oflh-website` on `main` at
`public/api/releases/<tag>/latest.json`. This happens during draft creation;
no manual workflow step is needed. `latest.json` is excluded from new GitHub
release assets. Detached `.sig` files and signed updater packages stay on GitHub;
Tauri reads the signature embedded in the manifest and downloads only the
installer from GitHub.

Publishing a stable release automatically runs `updater-website.yml`, which
promotes the latest published stable version to `public/api/latest.json`.
Drafts and RCs never replace that stable endpoint. The workflow can also be
rerun manually to recover a failed promotion. Versioned manifests remain
available independently of CI artifact retention. Amplify's existing static
website build deploys these committed files.

Before using this automation, set `OFLH_WEBSITE_TOKEN` in this application's
Actions secrets to a fine-grained token with Contents write permission for
`karimz1/oflh-website`. Both workflows use it to commit and push the website.
Deploy the website's seeded stable manifest before distributing an app with
the new endpoint. Keep website `/api/*.json` requests outside SPA rewrites and
use a short cache lifetime for `/api/latest.json`. Previously installed apps
continue using their embedded GitHub endpoint until updated; retain old release
manifests for those clients.

### Set up the updater keys

Run these commands from the repository root. You need Node.js and
GitHub CLI authenticated with permission to manage this repository's secrets.

```sh
npm --prefix crates/oflh-desktop/ui ci

umask 077
mkdir -p "$HOME/.config/oflh"
npm --prefix crates/oflh-desktop/ui run tauri -- signer generate \
  --ci -p "" \
  -w "$HOME/.config/oflh/updater.key"
```

This creates `updater.key` and `updater.key.pub`. The example uses an empty
password. Keep the private key outside Git and back it up securely. Do not
replace an existing key just to repeat the setup. To use a password, omit
`--ci -p ""` and enter it when the generator prompts.

Print the public key:

```sh
cat "$HOME/.config/oflh/updater.key.pub"
```

Copy the whole output and paste it into `plugins.updater.pubkey` in
`crates/oflh-desktop/tauri.conf.json`. Leave the updater endpoint unchanged.

Upload the private key to GitHub Actions:

```sh
gh secret set TAURI_SIGNING_PRIVATE_KEY \
  --repo karimz1/open-file-lock-handle \
  < "$HOME/.config/oflh/updater.key"
```

Existing installations trust the public key they were built with. Changing it
requires a planned migration signed with the old key, or a manual reinstall.
If the old private key is lost, those installations cannot accept updates signed
with a replacement key. See the [Tauri updater guide](https://v2.tauri.app/plugin/updater/).

### Signing checks in CI

All Desktop CI builds, including development pull requests and RCs, require the
real signing secret. The frontend job signs a small test payload and verifies
it against the app's public key before any native jobs start, catching missing,
invalid, or mismatched keys early. Fork pull requests without access to the
secret cannot pass Desktop CI. The Release workflow passes the signing secrets
explicitly to the reusable Desktop workflow.

## Updater regression coverage

Every normal CI run checks the website publishing script and tests RC commits,
draft rebuilds, stable promotion, repeat promotion, and rejection of malformed
metadata. These checks run on every pull request, supported branch push, manual
CI run, and release's reusable CI call. They commit and push only to a temporary
local Git repository, so they need no website token and work on fork PRs.
Native CLI jobs wait for these checks before building the platform matrix.

Native Desktop CI uses a loopback HTTP server and the real Tauri updater to
exercise a synthetic newer RC, current and older versions, endpoint failures,
malformed metadata, signed downloads, tampered payload rejection, and signed
version mismatch rejection. Windows
and macOS jobs also serve their actual packaged updater artifact and verify its
download against the public key embedded in the app. Browser tests cover install,
restart, failure, release-note fallback, and retry behavior.

The loopback HTTP override and synthetic signing fixtures are test-only; the
production endpoint remains HTTPS. Tests do not run installers or replace the
running application. End-to-end installation and restart still need native
release-candidate testing.

Updater signing does not replace macOS notarization or Windows code signing.
These are not configured, so installers still show OS warnings.
