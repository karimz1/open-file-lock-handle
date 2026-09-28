# OFLH Desktop

OFLH Desktop is a separate graphical frontend to the existing Rust inspection
engine. The CLI and Ratatui interface retain their current entry points.

## Implementation plan and architecture

The repository's reusable APIs are `oflh_core::Target`, `Identity`, `Snapshot`,
`search::{ProcessIndex, Query, Scratch}` and `oflh_platform::Backend`. Native
backends already validate birth identities before termination. They return atomic
snapshots, not incremental observations. The TUI worker's events and scheduling
remain specific to its event loop. Its presentation-independent file-plus-port
composition and identity-checked folder scan now live in `oflh-platform`, while
`Process::inspection_folder` in core provides the shared folder choice. Both
frontends call these helpers; no terminal input or rendering logic was moved.

`crates/oflh-desktop/src` contains a testable service, IPC DTOs, snapshot queries,
and the optional Tauri shell. `ui` contains React, TypeScript, Tailwind and Lucide.
There is no dependency on `oflh-tui` and no native inspection in JavaScript.

One worker owns the scanner. A single pending scan replaces older requests and
cancels active work cooperatively. Generation checks reject stale publications.
Immutable snapshots and cached search indices remain in Rust; queries return
bounded pages, and details load on demand. Tauri commands run blocking work away
from the UI thread. The frontend owns filters, sort, viewport, selection by full
identity, focused details, theme and session history. Refresh retains old rows
until a new snapshot is ready. There is no periodic polling or auto-refresh.

Paths remain native `PathBuf`s in Rust. Display strings are sanitized separately;
path actions resolve an opaque snapshot reference, never a displayed path.
Birth counters are encoded in string keys, avoiding JavaScript integer precision
loss. Confirmation tickets capture identities and action mode in Rust and are
consumed once. Native termination revalidates each lifetime; force is never an
automatic fallback. Unknown metrics remain unknown.

## Validation and distribution plan

Tests cover query semantics, paging, serialization, native-path references,
error mapping, stale generations, cancellation, and confirmation consumption.
Frontend checks cover selection and asynchronous state behavior. Existing
`cargo xtask check` remains mandatory. Desktop shell checks require platform
WebView development prerequisites; the optional `desktop` feature isolates those
from CLI-only builds. Native CI should test both architectures on all three OSes.
Tauri produces desktop packages separately from the six existing CLI artifacts;
CLI release assembly must not ingest desktop files. Signing and notarization
require release-owner credentials and are not implied by local packaging.

Windows results remain resource users, not proven lock owners. macOS retains the
existing documented signal/PID race. Linux visibility depends on procfs access.
Open files alone do not prove locks on any platform. The UI must show backend
coverage warnings and explicitly label unavailable information.

## Build prerequisites

Use the repository's Rust toolchain and Node.js 24. See the
[Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for native SDKs.
Windows needs Visual Studio C++ build tools and WebView2; macOS needs Xcode
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

The [README desktop quick start](../README.md#oflh-desktop-from-source) launches
the development app. For a production frontend and embedded-assets executable:

```sh
npm --prefix crates/oflh-desktop/ui ci
npm --prefix crates/oflh-desktop/ui run tauri -- build --no-bundle -- --locked
./target/release/oflh-desktop
```

On Windows, run `target\release\oflh-desktop.exe`. The Tauri CLI and Rust crates
are locked; the JavaScript API uses the matching 2.11 minor release. Plugin APIs
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
rendering remains enabled on Linux X11 systems without a detected NVIDIA driver;
Windows and macOS are unaffected.

## Desktop interactions

- Choose a file/folder, enter a path, or drop one native target anywhere in the
  window. Recent targets retain native paths for this session only.
- Processes shows one row per captured lifetime. File usages shows individual
  target-matching observations. It does not claim exhaustive per-process handle
  enumeration. Command lines are not exposed by the current core model and are
  therefore not shown.
- Search uses the shared Rust substring, wildcard and word-boundary matcher over
  the loaded snapshot. Refresh performs new system inspection. Lock evidence only
  retains observations with additional evidence; Windows sharing-conflict
  ownership remains unverified.
- Click selects a process; Ctrl/Cmd-click or Space toggles selection. Selection
  in the file-usage table applies to processes, including their other rows.
  Select all applies to all matching processes, not only the visible page.
- Arrow keys, Home/End and Enter navigate/open details. Shift-F10 or right-click
  opens context actions. Column separators can be dragged or resized with arrows.
- Ctrl/Cmd+O chooses a file; add Shift for a folder. Ctrl/Cmd+F focuses search;
  Ctrl/Cmd+R or F5 refreshes; Ctrl/Cmd+A selects all matching processes;
  Ctrl/Cmd+C copies selected process summaries. Text inputs retain ordinary
  selection/copy behavior. Escape clears selection/details or cancels a dialog.
- Ctrl/Cmd+1–4 switches Processes, File usages, Ports, and Recent targets.
  Ctrl+Tab / Ctrl+Shift+Tab cycles these workspaces. Ctrl/Cmd+, opens Settings.
  Ctrl/Cmd+Shift+D toggles details for the selected process. `/` focuses search;
  Escape leaves the input without clearing its filter. Workspace shortcuts do not
  intercept text editing or confirmation dialogs. Button hints and Settings list
  the shortcuts, using the platform’s Ctrl/Cmd convention.
- Path actions use snapshot references resolved in Rust. Copy returns original
  text; non-Unicode paths that cannot be represented losslessly as clipboard text
  produce an explicit error. Native drop, refresh and reveal retain `PathBuf`s.
- Confirmations list all selected identities, including hidden selections, and
  focus Cancel. They are single-use and retain their captured identities across
  refresh. After sending requests, Rust checks the captured process lifetimes for
  up to 1.5 seconds, then reports confirmed exit, still running, or verification
  unavailable. Permission errors never count as proof of exit. The table refreshes
  automatically and closes the stale details panel. Normal termination never
  escalates to force.
- Ancestry runs from the oldest captured parent down to the current process.
  The current process is labeled and initially selected. Click a bordered row
  to select its actions; the current-process label remains visible when selecting
  another ancestor.
- Select an ancestor in the details panel to terminate or force-terminate that
  captured parent through the same default-cancel confirmation. The focused tree
  retains its birth identities across refreshes. Stopping a parent can close its
  children or your session; protected or unavailable identities are not actionable.
- Click a row to select it and open its details; double-click or Enter also works.
  The panel icon opens/closes details for that observation without implying an
  inline dropdown. Table paths are target-relative when possible, otherwise filenames;
  the panel retains the full selected path and its original Rust action reference.
  Matching handles, local ports, and ancestry appear near the panel’s top.
- Settings lives at the sidebar’s bottom left and includes the author credit,
  project link, and Buy Me a Coffee support link. Donate opens the same support
  page; no payment happens inside OFLH.
- F5 refreshes and is displayed on the Refresh button; Ctrl/Cmd+R also works.
- The header’s Star on GitHub link opens the project in your default browser;
  it does not perform any GitHub account action.
- Drag the left edge of the process details panel to resize it. Focus the separator
  and use Left/Right (or Home/End) for keyboard resizing; double-click resets its
  width. The saved width is constrained to keep the table usable in small windows.
- On first launch, choose a theme with VS Code Dark preselected. Settings offers
  saved Light, System,
  Rider Dark (inspired by Rider), VS Code Dark, and OFLH Purple presets with
  clickable color previews. Purple uses the terminal demo’s palette. System uses
  VS Code Dark when the OS prefers dark mode. Existing neutral Dark preferences
  migrate to VS Code Dark.
  Settings also provides a saved 12–18 px interface font size (14 px by default).
  Table row heights scale with the text, and filters use larger focusable controls.
  No periodic refresh or background polling runs while idle. Coverage notices remain
  available alongside results. CPU is unavailable until valid native samples
  exist; unknown values are never rendered as zero.

Column filters combine with the main search and the captured process filter.
Open **Column filters** to restrict process name, exact PID, full path (local
address for ports), CPU percentage, memory in MiB, evidence category, and
access/relation (protocol/state for ports). Numeric minimum and maximum bounds
are inclusive; unavailable metrics do not match a numeric bound. Apply filters
runs the query in Rust before paging and Select All; Clear filters resets them.
File and port column filters are kept separately.

Global search includes full native paths, so a shared folder name can match every
row. Use the Process name column filter to isolate a named process. After
termination the owner scope, search, and column filters remain in place: a cleared
process should show an empty scoped view, not unrelated system ports.

The Ports view uses `PortIndex` and `PortQuery` from core: bare digits match
contiguous port-number fragments, while `port:8080` requires the exact port.
Combine terms such as `port:8080 tcp`, `udp`, `ipv6` or `pid:1234`. It lists global
local TCP listeners and bound UDP sockets; Target processes only restricts the
view to owners also referencing the loaded file target. Opening Ports before
choosing a file runs only port discovery. Subsequent file scans merge port data by
full birth identity, without treating port-only owners as file users.

Copy a port or IPv4/IPv6 endpoint through context actions. The details drawer can
show an owner's ports or inspect its captured working directory (falling back to
the executable parent). Folder inspection commits only if the original birth
identity remains visible. Unknown owners remain non-actionable. Local bindings
are not proof of external reachability; permissions, namespaces and container
forwarding limit visibility. The terminal port workflow remains unchanged.

Scans publish atomic snapshots, so results appear
when each scan finishes; cancellation is cooperative between native operations.

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

Browser tests use synthetic IPC fixtures only in `ui/tests`; production has no
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

For tagged builds, replace `dev` with the workspace version prefixed by `v`.
The separate desktop CI workflow feeds the existing draft release workflow;
CLI packages and Homebrew stay independent. Signing, notarization, installer
execution and native interaction review remain release gates, not implied by a
successful local compile. AppImage generation is not part of the current package
matrix; Linux currently uses DEB and RPM.

## Release candidate validation

See [Desktop RC validation](desktop-rc.md) for automated coverage, test reports,
screenshot regeneration, and native installation checks before distribution.
