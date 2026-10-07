# Architecture

oflh is a Cargo workspace. One scanner library serves two frontends: a terminal
app and a Tauri desktop app. Neither frontend has its own file or port scanner.

```text
              ┌──────────────┐      ┌──────────────────────────────┐
              │  oflh (CLI)  │      │  oflh-desktop                │
              │  args, start │      │  Rust service + Tauri shell  │
              └──────┬───────┘      │  React UI (ui/)              │
                     │              └──────────────┬───────────────┘
              ┌──────▼───────┐                     │
              │  oflh-tui    │                     │
              │  state, view │                     │
              └──────┬───────┘                     │
                     └───────────┬─────────────────┘
                          ┌──────▼────────┐
                          │ oflh-platform │  native APIs, unsafe lives here
                          └──────┬────────┘
                          ┌──────▼────────┐
                          │  oflh-core    │  targets, identities, search
                          └───────────────┘
```

| Crate | Owns | Must not contain |
| --- | --- | --- |
| [`oflh-core`](../crates/oflh-core) | Targets, process identities, observations, search, version rules, errors | OS calls, `unsafe` |
| [`oflh-platform`](../crates/oflh-platform) | Linux, macOS, and Windows backends; ports; metrics; termination; elevation; update HTTP client | UI state |
| [`oflh-tui`](../crates/oflh-tui) | Terminal state, input, rendering, background workers, translations | Native calls, `unsafe` |
| [`oflh`](../crates/oflh) | Argument parsing and startup of the `oflh` binary | Business logic, `unsafe` (`#![forbid(unsafe_code)]`) |
| [`oflh-desktop`](../crates/oflh-desktop) | Desktop service, Tauri shell (`desktop` feature), React frontend in `ui/` | A second scanner |
| [`xtask`](../xtask) | `cargo xtask check`, packaging, release assembly, updater signing | Anything shipped to users |

## The backend trait

Every platform implements `oflh_platform::Backend` (abridged; `is_running` has
a default that reports "unavailable"):

```rust
pub trait Backend: Send + 'static {
    fn scan(&mut self, target: &Target, cancel: &Cancellation) -> Result<Snapshot>;
    fn sample(&mut self, ids: &[Identity], cancel: &Cancellation)
        -> Result<Vec<(Identity, Metrics)>>;
    fn is_running(&mut self, id: Identity) -> Result<bool>;
    fn terminate(&mut self, id: Identity, force: bool, cancel: &Cancellation) -> Result<()>;
}
```

`oflh_platform::native()` returns the backend for the current OS. A backend is
owned by one worker thread and never shared. `scan` returns a `Snapshot` of
processes and their observations (path, relation, access, lock evidence) plus
coverage warnings. `sample` refreshes CPU and memory for known identities
without repeating file discovery.

## Scanning off the UI thread

A background worker owns the backend; the UI only handles input, filtering, and
drawing.

- Every request carries a generation number. A refresh supersedes older work,
  and results from older generations are discarded.
- Queues are bounded, and a new request replaces a pending one instead of
  piling up.
- Cancellation is checked between native calls. It cannot interrupt an OS call
  that is already running.
- The terminal redraws only when something changed; an idle screen costs
  nothing.
- Port discovery uses `netstat2` and joins socket owners to file results by
  process identity. A socket without a confirmed owner can never be an action
  target.

On Windows, folder scans run in a headless helper: the same executable started
with a hidden argument, placed in a kill-on-close job object, speaking a small
versioned binary protocol over pipes. If a native name query blocks, the parent
can stop the helper without stopping any inspected process. See
[inspection performance](inspection-performance.md#process-reference-windows-folder-inspection)
for details.

## Process identity and safety

A PID alone is not an identity, because the OS reuses PIDs. oflh identifies
every process by PID plus birth time (`Identity`). Metrics, selections, the
focused ancestry tree, and every action are bound to that pair.

- Linux signals through an owned `pidfd`. Windows force termination uses a
  validated process handle; normal termination posts `WM_CLOSE` to the
  process's windows. macOS re-checks the start time just before signaling,
  which leaves a narrow exit-and-reuse race the OS offers no way to close.
- Normal termination never silently escalates to force.
- Confirmation defaults to **Cancel** and lists selected processes that the
  current filter hides.
- `Identity::validate` refuses PID 0, PID 1, oflh's own process, and any
  process whose start time is unknown. Backends call it before every action, so
  the guard does not depend on the UI. (oflh also leaves itself out of scan
  results.)

## Native code boundaries

All `unsafe` code lives in `oflh-platform`. Each block has a `// SAFETY:`
comment, buffer lengths and record layouts are checked before reading, and OS
resources are owned by RAII guards. The workspace denies
`clippy::undocumented_unsafe_blocks` and `unsafe_op_in_unsafe_fn`.

Native paths are kept losslessly (`OsString`/`PathBuf`, including invalid UTF-8
or unpaired UTF-16). Control and formatting characters are sanitized only when
displayed. Errors keep the failing operation and the original OS error code.

Release builds keep `panic = "unwind"` so terminal and handle guards can restore
state. Tests, fixtures, benchmarks, and developer tools are never linked into the
shipped binaries.

## Desktop

The Rust service keeps the snapshot and its search index. The React frontend
asks for one page of at most 200 rows at a time plus the details of the selected
row, so a scan with tens of thousands of rows never crosses the IPC boundary in
full. The grid is virtualized.

IPC refers to processes and paths by keys scoped to the current process
lifetime and snapshot. Native actions resolve those keys on the Rust side
instead of trusting text from the frontend. A process action needs a
single-use confirmation ticket that captures the identities and the
normal/force mode; the backend re-validates identities before signaling.

### Desktop administrator recovery

When a termination fails with *permission denied*, the service keeps the denied
targets (with their identities) on the action receipt. **Retry with
administrator privileges** consumes that receipt and creates a new single-use
confirmation for the same targets and mode; the frontend cannot add targets.

After confirmation, the service starts the desktop executable itself in a
headless `--admin-terminate` mode, before any WebView is created, through the
platform's elevation mechanism:

| OS | Mechanism | Result channel |
| --- | --- | --- |
| Windows | UAC, owned helper process handle | Exit code |
| Linux | polkit `pkexec` | stdout |
| macOS | `osascript` administrator authorization | stdout |

The helper receives only numbers (PID, birth identity, requester PID, mode), so
there is no shell command to inject into. It re-validates identity and
protection with the normal backend and returns a bounded numeric outcome that
preserves the OS error code. There is no daemon and no privileged result file.
Cancelling the OS prompt stops the remaining targets. CI tests the helper's
identity guards and mode parsing on all six native targets; the interactive
consent dialogs are tested manually.

## Where to go next

- [Development](development.md): build, test, and project layout.
- [Regression coverage](inspection-regressions.md): the contracts that guard
  scanner changes.
- [Releasing](releasing.md): packaging and updater signing.
