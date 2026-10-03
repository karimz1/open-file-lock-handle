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
