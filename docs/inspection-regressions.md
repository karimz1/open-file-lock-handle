# Inspection and desktop regression coverage

Use these contracts when changing native discovery, scan scheduling or the desktop
grid. The performance work in [#59](https://github.com/karimz1/open-file-lock-handle/pull/59),
[#60](https://github.com/karimz1/open-file-lock-handle/pull/60),
[#61](https://github.com/karimz1/open-file-lock-handle/pull/61),
[#63](https://github.com/karimz1/open-file-lock-handle/pull/63),
[#65](https://github.com/karimz1/open-file-lock-handle/pull/65) and
[#67](https://github.com/karimz1/open-file-lock-handle/pull/67) was combined by
[#69](https://github.com/karimz1/open-file-lock-handle/pull/69).
[The integration report](https://github.com/karimz1/open-file-lock-handle/issues/68#issuecomment-6020352749)
records its original native validation. [Issue 77](https://github.com/karimz1/open-file-lock-handle/issues/77)
tracks the additional parser, native-buffer and snapshot regressions.

## Contracts and test locations

| Change or contract | Regression coverage | Test location |
| --- | --- | --- |
| #59: adaptive Windows resource batches and progress | Tiny directories share batches; self-only groups stop splitting; file-identity caches remain scan-local; fallback caps stay explicit; directory/file native users and progress are checked | `crates/oflh-platform/src/windows.rs`, `crates/oflh-platform/tests/native.rs` |
| Native inspection, cancellation and identity | Real file users, lock release, CPU/memory sampling, deleted replacements/hard links, protected processes, parent termination and an independent C lock fixture | `crates/oflh-platform/tests/native.rs` |
| #60: bounded desktop queries and responsive paging | Full 20,000-observation query cache reused across pages and selection; invalid queries rejected; file/port queries isolated; all 1,000 matching process lifetimes selected despite a 50-row final viewport | `crates/oflh-desktop/src/dataset.rs`, `query_cache_tests` |
| #61: column fitting | Enter leaves divider width unchanged; double-click fits; Fit all includes offscreen rows and skips hidden columns; changed filters abandon stale measurements | `crates/oflh-desktop/ui/tests/workspace.spec.ts` |
| #63: parallel Windows resource discovery | Every batch/file retained, native work overlaps within worker/queue bounds, cancellation disconnects blocked dispatch, errors/panics join all workers; explicit fallback cap warnings | `crates/oflh-platform/src/windows.rs`, unit tests |
| #65: uncapped Windows process-reference discovery | 160 distinct native users; a held file beyond 10,000 unused entries; read-only/write-only/metadata handles; directory handles; closed-file mappings; outside hard links; deleted-name evidence or an explicit warning; unacknowledged oplock breaks | `crates/oflh-platform/tests/native.rs`, Windows tests |
| #65: native ABI and owned-helper boundary | All 4,096 decoded handle records and pointer-width fields retained; malformed extents/counts rejected; lossless UTF-16 and birth identities; bounded IPC, cancellation/reaping, partial failures, unchanged-heartbeat stall limits | `crates/oflh-platform/src/windows/handles/snapshot.rs`, `inspection_protocol.rs`, `inspection_transport.rs` |
| #67: macOS workers | Each PID dispatched once; independent scans share a concurrency ceiling; errors, panics and waiting cancellation release only owned permits | `crates/oflh-platform/src/process_pool.rs` |
| #60: cold grid column sorts | Lazy relevance scoring preserves the same complete per-observation match set for every supported file-column sort, including both sort directions and offscreen selection | `crates/oflh-desktop/src/dataset.rs`, `query_cache_tests` |
| Linux fast process parsing | Nested/non-Unicode names, birth identity, exited states, truncated/overflowing metrics, unknown RSS, lossless mapping paths and malformed mapping evidence | `crates/oflh-platform/src/linux.rs`, unit tests |
| macOS descriptor initialization | Native returned bytes fit the allocation and contain whole ABI records before `set_len`; exact-full/empty/failed reads retain conservative partial coverage; invalid geometry and partial records fail | `crates/oflh-platform/src/native_buffer.rs`, called by `macos.rs` |
| CPU/memory lifetime isolation | CPU deltas never carry across a reused PID or an absent lifetime; zero/backwards clocks and counters remain unknown; later valid samples recover; only the full birth identity receives memory/CPU samples | `crates/oflh-platform/src/lib.rs`, `metric_identity_tests` |
| Snapshot lifetime isolation | A reused PID cannot inherit the old process key, selected scope, detail lookup or native path reference; the old immutable snapshot retains its own references | `crates/oflh-desktop/src/dataset.rs`, `query_cache_tests` |
| #74: familiar Windows paths | Display/copy text uses drive/UNC forms when equivalent; namespace-dependent names and long clipboard paths retain their prefix; original native references remain lossless, including non-Unicode paths | `crates/oflh-desktop/src/path_text.rs`, Windows dataset tests |
| #75: completed duration | Only successful accepted scans publish a duration; cancel/failure/stale work retains the previous value; ms/s/min formatting, localized/minimum-window footer | `crates/oflh-desktop/src/service.rs`, `ui/src/scanDuration.test.ts`, browser tests |
| #76: quiet automatic refresh | Old viewport remains until a matching page is ready; search/scroll/focus/details stay usable; one scan and page request at a time; no idle progress polling; no refresh timer restart before accepted rows; stale/reused identities and changed observations cannot gain actions | `crates/oflh-desktop/ui/tests/workspace.spec.ts` |

A 200-row IPC page limits one response, not native discovery. A 10,001-process
fixture requires the complete count and the last page to remain accessible even
after select-all reports its explicit 10,000-process safety limit. It does not
truncate the inspected dataset. Tests assert result membership and identity separately from
viewport sizes and timings.

## Run and review

```sh
cargo xtask check
cargo build --release --locked --bin oflh
npm --prefix crates/oflh-desktop/ui ci
npm --prefix crates/oflh-desktop/ui test
npm --prefix crates/oflh-desktop/ui run build
npm --prefix crates/oflh-desktop/ui run test:ui
npm --prefix crates/oflh-desktop/ui run format:check
```

`cargo xtask check` includes formatting, Clippy and every workspace test. Native
fixtures need permission to create processes, PTYs and loopback sockets. Investigate
a permission or timing failure; do not remove the assertion or skip the fixture.
Run browser suites against one local test server at a time.

The [CI workflow](../.github/workflows/ci.yml) runs native tests, repeated terminal
workflows, equivalent-coverage profiles and the tested release executable on Linux,
macOS and Windows, each on x86-64 and ARM64. The
[desktop workflow](../.github/workflows/desktop.yml) checks UI tests, desktop packages,
administrator recovery and updater behavior. Inspect the checks for the exact PR
head and the combined development branch before merging. Cross-compilation cannot
replace native execution. Review intentional terminal snapshots and synthetic UI
screenshots when rendering changes.

Native fixtures, C interoperability tools, profiling examples and dev-dependencies
remain outside the distributed application. The public test data uses synthetic
paths and process names; timing artifacts contain aggregate measurements.

## What the tests cannot establish

Coverage gates catch specific regressions; they cannot guarantee that every future
change or live system behaves correctly. Protected processes, permissions, native
name loss, network filesystems and filter drivers still affect discovery. Open
references are not proof of locks; Windows resource users are not proven lock
owners. Preserve partial-result warnings and unknown metrics.

Use [inspection performance](inspection-performance.md) for reproducible profiles
and equivalent-coverage speed comparisons. Shared-runner timings and changing
whole-disk observations are diagnostics, not a universal latency guarantee or a
comparison of operating-system speed. Further investigation remains recorded in
issues [56](https://github.com/karimz1/open-file-lock-handle/issues/56),
[57](https://github.com/karimz1/open-file-lock-handle/issues/57),
[58](https://github.com/karimz1/open-file-lock-handle/issues/58),
[62](https://github.com/karimz1/open-file-lock-handle/issues/62) and
[66](https://github.com/karimz1/open-file-lock-handle/issues/66).
