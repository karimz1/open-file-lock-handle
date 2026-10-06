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

## Testing the development inspection changes

The inspection PRs are integrated on `development` after combined native and
desktop validation. The maintainer can test that branch before promoting it to
`main`. Use a separate checkout to preserve other local work:

```sh
git clone --branch development https://github.com/karimz1/open-file-lock-handle.git oflh-development
cd oflh-development
cargo xtask check
cargo build --release --locked --bin oflh
```

For the desktop, install the prerequisites above, then run:

```sh
npm --prefix crates/oflh-desktop/ui ci
npm --prefix crates/oflh-desktop/ui run tauri -- build --no-bundle -- --locked
```

Launch `target/release/oflh-desktop` (`oflh-desktop.exe` on Windows). Useful manual
checks complement the native and browser regressions:

- Inspect a folder with known file users, then `/` on Linux/macOS or `C:\` on
  Windows. Check both the observed users and coverage warnings. Unused files are
  not traversed by healthy process-reference inspection; permissions and native
  name loss remain limitations.
- Set auto reload to five seconds and start an inspection long enough to see the
  progress dialog. F5, Ctrl/Command+R, dropped targets and automatic refresh must
  not restart it. The dialog shows elapsed time and attempted work with an unknown
  total. Explicit Cancel must release the dialog; refresh resumes after a full
  interval.
- With large results, change search and column filters, scroll, and use Home/End
  across pages. Rows and selection must belong to the current query and snapshot.
  A 200-row IPC page is not a result-count cutoff.
- Double-click a column divider to fit it. Enter on the divider must leave its
  width unchanged. Fit all columns must include offscreen matching rows, skip
  hidden columns and abandon stale measurements when the filter changes.

See [desktop usage](desktop-usage.md) for the controls and
[inspection performance](inspection-performance.md) for profiling commands and
evidence limits. Native Linux/macOS/Windows x86-64 and ARM64 CI validates the
combined branch; synthetic timings do not guarantee the same latency on every
machine. The development integration and its gates are tracked in
[issue 68](https://github.com/karimz1/open-file-lock-handle/issues/68).
