# Build locally

Run from the repository root with Rust installed. Desktop also needs Node.js,
installed frontend dependencies, and the [Tauri system prerequisites](https://v2.tauri.app/start/prerequisites/).

## CLI

```sh
cargo build --release --locked --bin oflh
./target/release/oflh
```

<a id="desktop-development"></a>
<a id="build-desktop"></a>

## Desktop

```sh
npm --prefix crates/oflh-desktop/ui run tauri -- build --no-bundle -- --locked
./target/release/oflh-desktop
```

This builds the desktop executable without an installer. On Windows, run the
corresponding `.exe` in `target\release`.

For auto-updater signing, see [Set up the updater keys](releasing.md#set-up-the-updater-keys).

## Validate changes

Run the workspace checks and, with the Desktop prerequisites installed, the
native updater regression tests:

```sh
cargo xtask check
cargo test -p oflh-desktop --features updater-tests --locked
```

Updater tests use a local HTTP server and synthetic signed payloads. Desktop CI
also validates each Windows/macOS updater package after bundling; see
[Updater regression coverage](releasing.md#updater-regression-coverage).

## Native inspection profiles

See [inspection performance](inspection-performance.md) for equivalent-coverage fixture comparisons, native CI artifacts and the meaning of progress counters.
