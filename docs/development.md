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
