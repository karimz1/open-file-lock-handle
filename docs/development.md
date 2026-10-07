# Development

How to build, run, and test oflh from a clone. Run every command from the
repository root.

- [Prerequisites](#prerequisites)
- [Build the CLI](#cli)
- [Build the desktop app](#desktop-development)
- [Run the checks](#validate-changes)
- [Project layout](#project-layout)
- [Terminal snapshots and translations](#terminal-snapshots-and-translations)
- [Profiling](#native-inspection-profiles)
- [Manual checks](#testing-the-development-inspection-changes)

## Prerequisites

| For | You need |
| --- | --- |
| CLI and all Rust tests | [rustup](https://rustup.rs). The pinned toolchain (`rust-toolchain.toml`, currently 1.98.1 with `rustfmt` and `clippy`) installs automatically on first use. |
| Desktop app | Node.js (CI uses 24) and the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your OS |
| Native tests | A C compiler (`cc`/`clang`, or MSVC on Windows) for the lock fixture, and permission to create processes, pseudo-terminals, and loopback sockets |

On Debian or Ubuntu, the desktop build needs:

```sh
sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev libxdo-dev libssl-dev librsvg2-dev patchelf
```

A [dev container](../.devcontainer/devcontainer.json) with Rust and Node.js is
included.

## CLI

```sh
cargo build --release --locked --bin oflh
./target/release/oflh .
```

`cargo run` also works; `oflh` is the workspace's default member. Local builds
report their version as `development` and never check for updates.

<a id="build-desktop"></a>
<a id="desktop-development"></a>

## Desktop

```sh
npm --prefix crates/oflh-desktop/ui ci
npm --prefix crates/oflh-desktop/ui run tauri -- build --no-bundle -- --locked
./target/release/oflh-desktop
```

This builds the frontend and the desktop executable without an installer. On
Windows the binary is `target\release\oflh-desktop.exe`. The Tauri shell is
behind the `oflh-desktop` crate's `desktop` feature, so plain
`cargo build --workspace` and `cargo test --workspace` do not need WebKitGTK.

Updater signing is only needed for release builds; see
[Set up the updater keys](releasing.md#set-up-the-updater-keys).

## Validate changes

One command runs the gates CI enforces for Rust code:

```sh
cargo xtask check
```

It runs, in order:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

For desktop frontend changes, also run:

```sh
npm --prefix crates/oflh-desktop/ui run format:check
npm --prefix crates/oflh-desktop/ui test          # Vitest unit tests
npm --prefix crates/oflh-desktop/ui run build     # TypeScript check + Vite build
npm --prefix crates/oflh-desktop/ui run test:ui   # Playwright (once: cd crates/oflh-desktop/ui && npx playwright install chrome)
```

And for updater changes, with the desktop prerequisites installed:

```sh
cargo test -p oflh-desktop --features updater-tests --locked
```

The updater tests use a loopback HTTP server and synthetic signed payloads;
they never contact the real update service.

Native tests start real processes that hold files, locks, and sockets. A
failure there usually means a real behavior change or a restricted
environment. Investigate it; do not skip or loosen the test. Changes to a
platform backend need native runs on Linux, macOS, and Windows for both x86-64
and ARM64, which [CI](../.github/workflows/ci.yml) provides on every pull
request. Cross-compiling is not a substitute.

[Regression coverage](inspection-regressions.md) lists which test guards which
behavior.

## Project layout

```text
crates/
  oflh-core/        targets, identities, observations, search (no OS calls)
  oflh-platform/    Linux, macOS, Windows backends; ports; updates; elevation
    tests/          native integration tests and the C lock fixture
    examples/       profiling harnesses (never shipped)
  oflh-tui/         terminal UI; tests/snapshots/ holds golden screens
  oflh/             the `oflh` binary: argument parsing and startup
  oflh-desktop/     desktop service (Rust) and Tauri shell
    ui/             React + TypeScript frontend, Vitest and Playwright tests
xtask/              cargo xtask: check, packaging, release assembly
docs/               user and contributor documentation
  measurements/     raw JSON from recorded benchmarks
.github/workflows/  ci.yml, desktop.yml, release.yml, and helpers
```

See [Architecture](architecture.md) for what belongs in each crate.

## Terminal snapshots and translations

The terminal UI is tested against golden screens in
`crates/oflh-tui/tests/snapshots/`. After an intentional layout change,
regenerate them and review every changed file:

```sh
OFLH_UPDATE_SNAPSHOTS=1 cargo test -p oflh-tui --locked
git diff crates/oflh-tui/tests/snapshots/
```

Set `OFLH_VISUAL_DIR=/some/dir` to also export SVG renderings for visual
review. Wide (CJK) glyphs must occupy their real terminal width.

The terminal app ships in English, German, and Simplified Chinese.
`crates/oflh-tui/src/messages.rs` holds the terminal strings and help;
`crates/oflh/src/messages.rs` holds `--help` and CLI errors. The formatting
macros compile every language variant with the same arguments, so a missing
argument is a compile error. Translate labels only; keep search tokens,
shortcuts, file names, and OS error text unchanged. Locale tests pass values in
or use child-process environments rather than mutating the test process's
environment.

Desktop translations live in `crates/oflh-desktop/ui/src/locales/`; see the
[translation guide](../crates/oflh-desktop/ui/src/locales/README.md).

<a id="native-inspection-profiles"></a>

## Profiling

Developer harnesses live in `examples/` and are excluded from shipped binaries:

| Command | Measures |
| --- | --- |
| `cargo run --release --locked -p oflh-platform --example inspection_profile` | Native scan of synthetic held-file fixtures |
| `cargo run --release --locked -p oflh-desktop --example grid_profile` | Desktop search and paging over 50,000 rows |
| `cargo run --release --locked -p oflh-tui --features profiling --example navigation_profile` | Terminal search, sorting, and drawing |

[Inspection performance](inspection-performance.md) explains how to compare a
change against a baseline and what the counters mean.

<a id="testing-the-development-inspection-changes"></a>

## Manual checks

Automated tests cover most behavior, but check these by hand after changing
scanning or the desktop grid:

1. Inspect a folder that a known process is using, then `/` (or `C:\`). Confirm
   the expected process appears and read the coverage warnings.
2. Turn on five-second auto refresh and start a long scan. `F5`, `Ctrl+R`,
   dropping a new target, and the auto timer must not restart it. **Cancel**
   must close the progress panel, and auto refresh must wait a full interval
   before running again.
3. With a large result, change the search and column filters, scroll, and press
   `Home`/`End`. Rows and selection must always match the current query.
4. Double-click a column divider to fit it; pressing `Enter` on a divider must
   not change its width. **Fit all columns** must account for off-screen rows
   and skip hidden columns.
