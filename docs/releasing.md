# Releasing

Maintainer reference for cutting a release, publishing updater metadata, and
managing signing keys. For local builds, see [Development](development.md).

- [Overview](#overview)
- [Prepare a release](#prepare-a-release)
- [Review and publish the draft](#review-the-draft)
- [What gets built](#desktop-packages)
- [Homebrew](#homebrew)
- [Desktop updates](#desktop-updates-and-signing)
- [Set up the updater keys](#set-up-the-updater-keys)
- [Updater regression coverage](#updater-regression-coverage)
- [CI artifact storage](#ci-artifact-storage)

## Overview

Pushing a `v*` tag runs the [Release workflow](../.github/workflows/release.yml).
It builds and tests the CLI and desktop app on six native runners (Linux, macOS,
Windows × x86-64, ARM64), assembles the downloads, `checksums.txt`, and
`latest.json`, and creates a **draft** GitHub release. It never publishes on its
own and refuses to overwrite a published release.

## Prepare a release

1. Set `workspace.package.version` in `Cargo.toml` and run `cargo check` to
   update `Cargo.lock`.
2. Run `cargo xtask check`, commit, and wait for CI to pass.
3. Tag the reviewed commit with the same version, prefixed by `v`, and push the
   tag:

   ```sh
   git tag v0.8.0-rc.1
   git push origin v0.8.0-rc.1
   ```

A suffix such as `-rc.1` makes a prerelease. Prereleases are never marked
**Latest**, so the website, the updater, and Homebrew keep pointing at the last
stable version. Release binaries report the tag without the `v`; local builds
report `development`.

To rerun the workflow for an existing tag:

```sh
gh workflow run release.yml --ref v0.8.0-rc.1 -f tag=v0.8.0-rc.1
```

Never move a published tag.

<a id="review-and-publish-the-draft"></a>

## Review the draft

1. Confirm every native job passed for the tagged commit and all expected assets
   are attached.
2. Edit the notes. New drafts start with a
   [download introduction](../.github/release-notes/downloads.md) above GitHub's
   generated changelog. Retrying a draft keeps your edits.
3. Test anything native the release changes. Browser tests use sample data and
   do not exercise native dialogs or installers.
4. Publish. Publish release candidates as prereleases.

## Desktop packages

| OS | Packages |
| --- | --- |
| Linux | `.deb`, `.rpm`, `.tar.gz` |
| macOS | `.dmg`, plus `.app.tar.gz` and `.sig` for the updater |
| Windows | NSIS `-installer.exe` and `.sig` (no portable build) |

CI tests the desktop executable and then bundles that same binary with
`tauri bundle --no-binary-patching`. `cargo xtask assemble-release` collects the
six artifact sets, validates their receipts, versions, and hashes, rejects
missing or unexpected files, and writes the downloads, `checksums.txt`, and the
updater manifest. Exact packaging commands live in the
[Desktop workflow](../.github/workflows/desktop.yml).

## Homebrew

Publishing a release triggers the [Homebrew workflow](../.github/workflows/homebrew.yml),
which dispatches `update-oflh.yml` in
[karimz1/homebrew-tap](https://github.com/karimz1/homebrew-tap). That workflow
verifies downloads against the release checksums and updates the `oflh-cli`
formula and `oflh-desktop` cask. It also runs daily for the latest stable
release. The dispatch needs the `HOMEBREW_TAP_TOKEN` secret (a fine-grained token
with Actions write access to the tap).

After publishing, check that the tap workflow succeeded and that
`brew info karimz1/tap/oflh-cli` shows the new version.

## Desktop updates and signing

Desktop apps and the CLI's release notice read
`https://oflh.karimzouine.com/api/latest.json`, falling back to the
`latest.json` attached to GitHub's latest release. A valid website response
wins. Linux `.deb` and `.rpm` users update through their package manager or the
release page.

How the manifest gets there:

1. While creating the draft, the Release workflow commits the assembled
   `latest.json` (RCs included) to `karimz1/oflh-website` at
   `public/api/releases/<tag>/latest.json`, and attaches the same file to the
   GitHub release.
2. Publishing a **stable** release runs
   [`updater-website.yml`](../.github/workflows/updater-website.yml). It
   downloads and validates that release's `latest.json`, writes it to
   `public/api/releases/<tag>/latest.json` and `public/api/latest.json`, and
   repairs missing or stale draft metadata. Drafts and RCs never touch the
   stable endpoint. Rerun it manually to recover a failed promotion.
3. The website's existing static build deploys the committed files.

Keep attaching `latest.json` to every GitHub release: apps installed before
0.7.0, and the fallback path in newer ones, read GitHub's latest-release URL.
Signatures are embedded in the manifest; the signed packages and their `.sig`
files stay on GitHub.

Requirements:

- `OFLH_WEBSITE_TOKEN`: fine-grained token with Contents write access to
  `karimz1/oflh-website`, used by both workflows.
- `TAURI_SIGNING_PRIVATE_KEY`: the updater signing key (below).
- Website `/api/*.json` must be excluded from SPA rewrites, and
  `/api/latest.json` should have a short cache lifetime.

Updater signing is not OS code signing. Windows Authenticode and macOS
notarization are not configured, so installers still trigger OS warnings.

### Set up the updater keys

You only need this once. Do not replace an existing key: installed apps trust
the public key they were built with, and if the private key is lost they can no
longer accept updates. Changing keys requires a migration release signed with
the old key, or users reinstalling manually. See the
[Tauri updater guide](https://v2.tauri.app/plugin/updater/).

Requires Node.js and an authenticated GitHub CLI that can manage repository
secrets.

```sh
npm --prefix crates/oflh-desktop/ui ci

umask 077
mkdir -p "$HOME/.config/oflh"
npm --prefix crates/oflh-desktop/ui run tauri -- signer generate \
  --ci -p "" \
  -w "$HOME/.config/oflh/updater.key"
```

This writes `updater.key` and `updater.key.pub` with an empty password. To set
a password, drop `--ci -p ""` and answer the prompt. Keep the private key out of
Git and back it up.

Paste the contents of `updater.key.pub` into `plugins.updater.pubkey` in
`crates/oflh-desktop/tauri.conf.json`, leaving the endpoint unchanged. Then
upload the private key:

```sh
gh secret set TAURI_SIGNING_PRIVATE_KEY \
  --repo karimz1/open-file-lock-handle \
  < "$HOME/.config/oflh/updater.key"
```

Every desktop CI run, including pull requests and RCs, signs a test payload and
verifies it against the public key before the native jobs start, so a missing
or mismatched key fails fast. Pull requests from forks cannot access the secret
and therefore cannot pass desktop CI.

## Updater regression coverage

On every CI run (pull requests from forks included), the website-publishing
script is tested against a temporary local Git repository: RC commits, draft
rebuilds, stable promotion, repeated promotion, and malformed metadata. No
token is needed, and native CLI jobs wait for these checks.

Native desktop CI runs the real Tauri updater against a loopback HTTP server
with synthetic manifests: newer RC, same and older versions, endpoint failures,
malformed metadata, signed downloads, tampered payloads, and signed version
mismatches. On Windows and macOS it also serves the actual packaged updater
artifact and verifies it against the app's embedded public key. Browser tests
cover install, restart, failure, release-note fallback, and retry.

These tests do not run installers or replace the running app. Test end-to-end
installation and restart by hand with a release candidate.

## CI artifact storage

Push and pull-request builds skip optional uploads (CLI binaries, macOS and
Windows desktop packages, validation receipts, browser reports). Linux `.deb`,
`.rpm`, and `.tar.gz` files are still uploaded because the install smoke tests
download them. All Actions artifacts expire after one day.

To get downloadable builds from a branch, run **CI** or **Desktop** manually with
`upload_artifacts` enabled. For builds you want to keep, push an RC tag: files
attached to a release do not expire. If artifacts expired before a retry, rerun
the whole Release workflow.
