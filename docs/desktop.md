# OFLH Desktop — developer reference

For using the application, start with the [Desktop user guide](desktop-usage.md).
For source builds, see [development](development.md#desktop-development).

OFLH Desktop is a separate graphical frontend to the existing Rust inspection
engine. The CLI and Ratatui interface retain their current entry points.

See [Desktop architecture](architecture.md#desktop-frontend) for the service,
worker, IPC, and process-identity design. This page covers source builds,
validation, and packaging only.

## Validation and distribution plan

Tests cover query semantics, paging, serialization, native-path references,
error mapping, stale generations, cancellation, and confirmation consumption.
Frontend checks cover selection and asynchronous state behavior. Existing
`cargo xtask check` remains mandatory. Desktop shell checks require platform
WebView development prerequisites. The optional `desktop` feature isolates those
from CLI-only builds. Native CI should test both architectures on all three OSes.
Tauri produces desktop packages separately from the six existing CLI artifacts.
CLI release assembly must not ingest desktop files. Signing and notarization
require release-owner credentials and are not implied by local packaging.

Windows results remain resource users, not proven lock owners. macOS retains the
existing documented signal/PID race. Linux visibility depends on procfs access.
Open files alone do not prove locks on any platform. The UI must show backend
coverage warnings and explicitly label unavailable information.

## Build prerequisites

Use the repository's Rust toolchain and Node.js 24. See the
[Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for native SDKs.
Windows needs Visual Studio C++ build tools and WebView2. macOS needs Xcode
Command Line Tools. The existing native scanner also needs the C/libclang tools
described in [development](development.md).

On Fedora, install:

```sh
sudo dnf install gtk3-devel webkit2gtk4.1-devel openssl-devel librsvg2-devel libappindicator-gtk3-devel patchelf
```

On Ubuntu 24.04, install:

```sh
sudo apt-get install build-essential libwebkit2gtk-4.1-dev libssl-dev libxdo-dev librsvg2-dev libappindicator3-dev patchelf
```

See [Desktop source builds](development.md#desktop-development) for development
and release executable commands.


On Windows, run `target\release\oflh-desktop.exe`. The Tauri CLI and Rust crates
are locked. The JavaScript API uses the matching 2.11 minor release. Plugin APIs
are invoked only from Rust, so plugin JavaScript packages are unnecessary.

### Linux graphics troubleshooting

OFLH checks for a Wayland session (`WAYLAND_DISPLAY` or `XDG_SESSION_TYPE`)
and a loaded NVIDIA driver (`/sys/module/nvidia/version` or
`/proc/driver/nvidia/version`) before GTK starts. If either is detected and no
explicit renderer preference exists, it enables the WebKit compatibility renderer.
The [upstream WebKit issue](https://bugs.webkit.org/show_bug.cgi?id=280210)
describes the startup failure this avoids. Detection is conservative: a loaded
NVIDIA driver on a hybrid-GPU machine also selects compatibility mode.

Startup uses a safe `exec` replacement with the same executable, arguments and
PID, setting `WEBKIT_DISABLE_DMABUF_RENDERER=1` in the new process environment.
It does not mutate the environment after threads start, create a helper process,
or alter global settings. Explicit values are always respected, including `0`.
Ordinary launches therefore need no environment prefix on detected systems.

To override the policy for diagnosis:

```sh
# Force compatibility rendering, even when detection does not apply:
WEBKIT_DISABLE_DMABUF_RENDERER=1 ./target/release/oflh-desktop
# Opt out and let WebKit use its default renderer:
WEBKIT_DISABLE_DMABUF_RENDERER=0 ./target/release/oflh-desktop
```

The same overrides work with `npm --prefix crates/oflh-desktop/ui run tauri -- dev`.
This affects rendering, not inspection or process-action semantics. Default
rendering remains enabled on Linux X11 systems without a detected NVIDIA driver.
Windows and macOS are unaffected.

For controls, shortcuts, filtering, recent targets, and process actions, see the
[Desktop user guide](desktop-usage.md). For platform discovery limits, see
[platform support](platform-support.md).

## Checks

From the repository root:

```sh
cargo xtask check
npm --prefix crates/oflh-desktop/ui run format:check
npm --prefix crates/oflh-desktop/ui run build
npm --prefix crates/oflh-desktop/ui test
npm --prefix crates/oflh-desktop/ui exec -- playwright install chrome
npm --prefix crates/oflh-desktop/ui run test:ui
cargo clippy -p oflh-desktop --all-targets --features desktop --locked -- -D warnings
cargo test -p oflh-desktop --features desktop --locked
```

Browser tests use synthetic IPC fixtures only in `ui/tests`. Production has no
mock data or browser fallback. Screenshots in `ui/test-results` are ignored and
should be reviewed for both themes and confirmation focus. The large-table test
uses 1,500 rows and asserts fewer than 60 rendered rows. This verifies rendering
bounds, not a claimed scanner speedup. Native integration tests discover and
force-terminate only a child created by the test harness, through the actual
service and native backend.

## Packages

After building the release executable, bundle without modifying it:

```sh
# Linux
npm --prefix crates/oflh-desktop/ui run tauri -- bundle --no-binary-patching --bundles deb,rpm
# macOS
npm --prefix crates/oflh-desktop/ui run tauri -- bundle --no-binary-patching --bundles dmg
# Windows
npm --prefix crates/oflh-desktop/ui run tauri -- bundle --no-binary-patching --bundles nsis
```

Outputs are under `target/release/bundle`. To collect a Linux x86-64 candidate:

```sh
cargo xtask package-desktop --version dev --os linux --arch amd64 --binary target/release/oflh-desktop --bundle-dir target/release/bundle --output desktop-dist
```

Collection refuses to overwrite existing collected artifacts. Use an empty
output directory for each candidate. After downloading all six native package
sets into `desktop-dist`, verify and generate aggregate checksums with:

```sh
cargo xtask assemble-desktop --version dev --output desktop-dist
```

For tagged builds, use the release tag, set `OFLH_VERSION` to its version without
`v`, and pass that same version through Tauri `--config` to both build and bundle.
The workflow generates this override automatically. See [release assembly](releasing.md)
for direct downloads, internal receipts and the single public checksum file.
The separate desktop CI workflow feeds the existing draft release workflow.
CLI packages and Homebrew stay independent. Signing, notarization, installer
execution and native interaction review remain release gates, not implied by a
successful local compile. AppImage generation is not part of the current package
matrix. Linux currently uses DEB and RPM.

### Arch Linux AUR

`packaging/arch/PKGBUILD` contains a source-built AUR recipe pinned to the
`v0.2.0` release. It builds against the host's WebKitGTK rather than extracting a
DEB or RPM. This recipe is not an official Arch repository package or a published
AUR entry. For a new upstream release, update `pkgver`, the release archive
checksum, and `pkgrel` as needed. Validate on Arch with
`makepkg --syncdeps --cleanbuild` and regenerate `.SRCINFO` before publishing to
the AUR.

## Release candidate validation

See [Desktop RC validation](desktop-rc.md) for automated coverage, test reports,
screenshot regeneration, and native installation checks before distribution.
