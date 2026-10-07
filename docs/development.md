# Building and testing OFLH

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

See [inspection regression coverage](inspection-regressions.md) for the contracts,
test locations and six-target validation required when changing native discovery
or desktop paging.

## Native inspection profiles

See [inspection performance](inspection-performance.md) for equivalent-coverage fixture comparisons, native CI artifacts and the meaning of progress counters.

<a id="testing-the-development-inspection-changes"></a>

## Manual inspection checks

After building the interface you changed, check these behaviors alongside the
native and browser regressions:

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
- Double-click a column divider to fit it. Enter on the divider must leave its
  width unchanged. Fit all columns must include offscreen matching rows, skip
  hidden columns and abandon stale measurements when the filter changes.

See [desktop usage](desktop-usage.md) for the controls and
[inspection performance](inspection-performance.md) for profiling commands and
evidence limits. Native discovery changes require Linux, macOS, and Windows
x86-64 and ARM64 validation.

### Terminal language regressions

The CLI/TUI supports English, German and Simplified Chinese through `--language`.
`crates/oflh-tui/src/messages.rs` owns terminal labels, literal format translations
and readable help catalogs; `crates/oflh/src/messages.rs` owns CLI help. Format macros
compile all language variants with the same arguments and evaluate only the selected
branch. Translate presentation, retaining native data and stable search tokens.

Locale tests use supplied values or child-process environments, never process-wide
unsafe environment mutation. Real PTY workflows pin English for deterministic
assertions; separate real CLI tests cover explicit languages and environment priority.
Multilingual fixtures cover narrow/wide grids, details, help and default-cancel actions.
When intentionally changing layouts, update snapshots and inspect every changed fixture
and optional `OFLH_VISUAL_DIR` SVG. Wide glyphs occupy their actual terminal columns.
The Windows user-locale boundary must also pass native x86-64/ARM64 execution.

### Terminal release-check design

The TUI has its own bounded worker; it never uses the scanner queue for HTTP.
`oflh-core::releases` owns version rules, `oflh-platform::updates` owns the verified
HTTPS request and bounded parsing, and the terminal owns notice/schedule state.
The fixed endpoint matches desktop; the displayed link is always the official
release page, never a URL from JSON. This is a notification, with manual install.

Use [SemVer precedence](https://docs.rs/semver/latest/semver/struct.Version.html#method.cmp_precedence)
when comparing releases: ordinary version ordering includes build metadata, which
must not create an update. The transport uses
[reqwest request timeouts and redirect policy](https://docs.rs/reqwest/latest/reqwest/blocking/struct.ClientBuilder.html)
and its maintained default Rustls provider with OS certificate verification.
Automatic errors cause no modal or redraw; explicit checks report errors locally.
`--no-update-check`, development and PR artifacts create no network worker.
Review intentional help snapshot changes and optional `OFLH_VISUAL_DIR` exports.
