# Regression coverage

These are the behaviors that scanner, scheduling, and desktop-grid changes
must preserve, and the tests that enforce them. If a change makes one of these
tests fail, the change is wrong until proven otherwise: investigate, don't
relax the assertion.

## Run the suites

```sh
cargo xtask check                                  # fmt, clippy, all Rust tests
cargo build --release --locked --bin oflh          # the binary CI packages
npm --prefix crates/oflh-desktop/ui ci
npm --prefix crates/oflh-desktop/ui run format:check
npm --prefix crates/oflh-desktop/ui test
npm --prefix crates/oflh-desktop/ui run build
npm --prefix crates/oflh-desktop/ui run test:ui    # Playwright
```

Native tests need permission to create processes, pseudo-terminals, and
loopback sockets. Run one browser suite at a time; they share a local test
server.

[CI](../.github/workflows/ci.yml) runs native tests, repeated terminal
workflows, equivalent-coverage profiles, and the release executable on Linux,
macOS, and Windows for x86-64 and ARM64.
[Desktop CI](../.github/workflows/desktop.yml) adds UI tests, packaging,
administrator recovery, and updater checks. Before merging, check the results
for the exact pull request head.

## Native discovery

| Contract | Test location |
| --- | --- |
| Real file users are found; locks are released when the holder exits; CPU and memory are sampled; deleted and replaced files, hard links, protected processes, and parent termination behave correctly, checked against an independent C lock fixture | `crates/oflh-platform/tests/native.rs` |
| Windows: 160 distinct users of one file; a held file behind 10,000 unused entries; read-only, write-only, and metadata-only handles; directory handles; mappings after the file handle closed; outside hard links; deleted names reported or explicitly warned about; an unacknowledged oplock break does not stall or create a false conflict | `crates/oflh-platform/tests/native.rs` (Windows) |
| Windows handle snapshot: all 4,096 decoded records and pointer-width fields kept; malformed extents and counts rejected; lossless UTF-16; bounded IPC; cancellation and reaping; partial failures; unchanged heartbeats do not reset the stall limit | `crates/oflh-platform/src/windows/handles/snapshot.rs`, `inspection_protocol.rs`, `inspection_transport.rs` |
| Windows Restart Manager fallback: tiny folders share batches; self-only groups stop splitting; identity caches are per scan; the 10,000-file cap is disclosed; parallel batches are all kept, workers are bounded, and errors or panics join every worker | `crates/oflh-platform/src/windows.rs` |
| Linux `/proc` parsing: nested and non-Unicode names, birth identity, exited states, truncated or overflowing metrics, unknown RSS, lossless mapping paths, malformed mapping lines | `crates/oflh-platform/src/linux.rs` |
| macOS descriptor buffers: returned bytes fit the allocation and hold whole records before use; full, empty, and failed reads keep conservative coverage; invalid geometry fails | `crates/oflh-platform/src/native_buffer.rs` |
| macOS workers: each PID dispatched once; concurrent scans share one ceiling; errors, panics, and cancellation release only their own permits | `crates/oflh-platform/src/process_pool.rs` |
| Ports: TCP and UDP owners on IPv4 and IPv6 are found and disappear after release | `crates/oflh-platform/tests/ports.rs` |

## Identity and metrics

| Contract | Test location |
| --- | --- |
| CPU deltas never carry across a reused PID; zero or backwards clocks stay unknown; later valid samples recover; only the exact birth identity receives samples | `crates/oflh-platform/src/lib.rs` (`metric_identity_tests`) |
| A reused PID cannot inherit the previous process's key, scope, details, or path references; old snapshots keep their own | `crates/oflh-desktop/src/dataset.rs` (`query_cache_tests`) |

## Desktop grid and refresh

| Contract | Test location |
| --- | --- |
| One cached 20,000-observation query is reused across pages and selection; invalid queries are rejected; file and port queries are isolated; all 1,000 matching processes are selected even when the last page shows 50 rows | `crates/oflh-desktop/src/dataset.rs` (`query_cache_tests`) |
| Every column sort, in both directions, returns the same complete match set with lazy relevance scoring | `crates/oflh-desktop/src/dataset.rs` (`query_cache_tests`) |
| A 10,001-process result keeps its full count and last page reachable; select-all reports its 10,000-process safety limit without truncating the data | `crates/oflh-desktop/src/dataset.rs` |
| Column fitting: `Enter` on a divider changes nothing; double-click fits; **Fit all** includes off-screen rows, skips hidden columns, and drops stale measurements | `crates/oflh-desktop/ui/tests/workspace.spec.ts` |
| Windows paths display as drive/UNC where equivalent; namespace-dependent names and long copied paths keep `\\?\`; native references stay lossless | `crates/oflh-desktop/src/path_text.rs` |
| Only successful scans update the "last scan took" duration; ms/s/min formatting | `crates/oflh-desktop/src/service.rs`, `ui/src/scanDuration.test.ts` |
| Background refresh keeps the old rows until matching new ones arrive; search, scroll, focus, and details stay usable; one scan and one page request at a time; stale identities never gain actions | `crates/oflh-desktop/ui/tests/workspace.spec.ts` |

A 200-row page limits one IPC response, not discovery. Tests assert result
membership and identity separately from viewport sizes and timings.

## Terminal

| Contract | Test location |
| --- | --- |
| 10,001-process navigation and selection; per-observation matching for every sort; identity-bound metric updates; cancelled index builds are discarded; details survive reload; auto refresh pauses in dialogs, search, and the tree | `crates/oflh-tui/src/tests.rs` |
| Golden screens in English, German, and Chinese at wide and compact sizes, including default-cancel confirmations | `crates/oflh-tui/tests/snapshots/` |
| Real PTY workflows of the shipped binary; explicit `--language` and locale environment priority | `crates/oflh/tests/terminal.rs` |
| Release notice: SemVer precedence ignores build metadata; RCs; invalid versions; HTTP status, redirect, timeout, oversized and malformed manifests over loopback; the update worker never blocks inspection; one footer row changes | `crates/oflh-core/src/releases.rs`, `crates/oflh-platform/src/updates.rs`, `crates/oflh-tui/src/tests.rs` |

No test depends on the public update service.

## What these tests cannot prove

They catch known regressions. They cannot prove that every live system
behaves: protected processes, permissions, lost native names, network file
systems, and filter drivers still limit discovery. Open references are not
locks, and Windows resource users are not proven lock owners, so keep
partial-result warnings and unknown metrics intact.

For speed comparisons use [inspection performance](inspection-performance.md).
Background and design decisions are recorded in the (closed) investigations
[#56](https://github.com/karimz1/open-file-lock-handle/issues/56),
[#57](https://github.com/karimz1/open-file-lock-handle/issues/57),
[#58](https://github.com/karimz1/open-file-lock-handle/issues/58),
[#62](https://github.com/karimz1/open-file-lock-handle/issues/62),
[#66](https://github.com/karimz1/open-file-lock-handle/issues/66).
