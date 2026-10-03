# Architecture

Both interfaces use the same Rust search and native inspection code. Neither UI
implements its own file or port scanner.

| Crate | Responsibility |
| --- | --- |
| `oflh-core` | Targets, process identities, observations, search, and errors |
| `oflh-platform` | Native discovery, metrics, and process actions |
| `oflh-tui` | Terminal state, input, rendering, and background work |
| `oflh` | CLI arguments and application startup |
| `oflh-desktop` | Desktop service, Tauri shell, and React frontend |
| `xtask` | Checks and release packaging |

## Scanning

A background worker owns the native backend. The UI handles selection, filters,
and rendering. Requests have generation numbers; refresh cancels old work and
results from older generations are ignored. Queues are bounded and repeated
requests replace pending work. Cancellation is checked between native calls;
it cannot interrupt every OS operation.

Metrics can be sampled without repeating file discovery. Idle terminal screens
redraw only when something changes. Port discovery uses `netstat2` and joins owners
to file results by PID and birth identity. Unknown owners cannot be action targets.

## Process safety

A PID alone is not an identity. Actions and metrics also use process birth time
so a reused PID cannot silently select a different process. Selection and focused
ancestry retain captured identities across refreshes. Confirmation defaults to
Cancel and includes selected processes hidden by filters.

Linux uses owned pidfds. Windows force termination uses a validated process handle;
normal termination sends window-close requests. macOS validates start time before
signaling, but lacks a pidfd equivalent and retains a narrow exit/PID-reuse race.
Normal termination never silently escalates to force termination.

## Native boundaries

Unsafe native code belongs in `oflh-platform`, with checked buffer lengths,
documented ABI assumptions, and owned resources released through RAII. Core,
terminal UI, and CLI remain safe Rust. Errors retain operation context and original
OS codes. Native paths remain lossless; sanitization applies only to display text.

Open-file observations do not prove locks. Missing metrics stay unknown and
incomplete scans carry warnings. See [Platform support](platform-support.md) for
backend coverage. Terminal cleanup uses drop guards; release builds retain unwinding.
Tests, fixtures, benchmarks, and developer tooling are separate from app binaries.

## Desktop

The desktop service keeps snapshots and search indices in Rust. React requests
bounded result pages and details rather than receiving the whole dataset. Tauri
shell dependencies are behind the `desktop` feature.

IPC uses process-lifetime keys and snapshot-scoped path references. Native actions
resolve these references rather than trusting displayed paths. Single-use
confirmation tickets capture process identities and action mode; the backend
revalidates identity before signaling.

See [Development](development.md) to build and test, and [Releasing](releasing.md)
for packaging.

## Desktop administrator recovery

The desktop service retains permission-denied targets with their original PID and
birth identity under the completed action receipt. An administrator retry consumes
that receipt and creates a new single-use confirmation with the same normal/force
mode. Neither the frontend selection nor a refreshed dataset supplies the targets.
Only an explicit confirmation launches the headless `--admin-terminate` mode of
the current desktop executable, before renderer initialization.

Native authorization stays in `oflh-platform`: UAC via an owned helper process
handle on Windows, polkit `pkexec` on Linux, and `osascript` administrator
authorization on macOS. Executable paths are passed losslessly on Windows/Linux;
macOS rejects non-UTF-8 executable paths rather than changing them. Shell and
AppleScript quoting protect the macOS executable path. Numeric-only arguments
carry the full identity, requester PID, and explicit mode. The requester remains
protected despite the helper having a separate PID. Native backends revalidate
birth identity and retain their existing native handle and signal invariants.

The helper returns a bounded numeric outcome, preserving native OS error codes.
Unix carries it over stdout; Windows uses the owned process's exit code. No
privileged result files, persistent daemons, or arbitrary shell commands are
accepted. Authorization cancellation stops subsequent targets. Exit checks remain
in the unprivileged service, so they may report unknown state. The six native CI
jobs test helper identity guards, mode parsing and force termination; Unix jobs
also check a denied different-user request followed by a privileged request.
Interactive consent dialogs require manual OS testing.
