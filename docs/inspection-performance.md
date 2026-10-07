# Inspection performance

oflh finds file users by asking the OS which files each process references,
not by walking the target folder. Scan time therefore depends on how many
processes and open handles exist, not on how many files are on disk. This page
records the measurements behind that claim, explains how to reproduce them, and
documents the implementation choices that make it work.

- [Recorded results](#recorded-results)
- [Reproducing the measurements](#native-comparison)
- [What the counters mean](#what-the-counters-mean)
- [Windows folder backend](#process-reference-windows-folder-inspection)
- [Windows single files and fallback](#windows-single-files-and-fallback)
- [macOS process workers](#macos-process-workers-and-phase-profiling)
- [Desktop grid](#desktop-search-and-navigation)
- [Terminal navigation](#terminal-navigation-and-search)

## Recorded results

All numbers below come from the JSON files in
[`measurements/`](measurements/). They describe the machines they ran on;
they are not guarantees, and the Windows and Linux rows ran on different
hardware, so they do not rank operating systems.

### Whole drive or root

| Platform | Target | Time | Visible users | References visited | Warnings | Data |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| Windows x64 (GitHub runner) | `C:\` | 352.474 ms | 137 | 8,754 | 10 | [JSON](measurements/inspection-windows-x64-2026-10-06.json) |
| Windows ARM64 (GitHub runner) | `C:\` | 509.999 ms | 144 | 10,241 | 9 | [JSON](measurements/inspection-windows-arm64-2026-10-06.json) |
| Linux x86-64 (local) | `/` | 194.668 ms | 153 | 69,723 | 2 | [JSON](measurements/inspection-linux-2026-10-05.json) |

The Linux scan returned 48,647 usages and traversed no directories. Each is a
single run of the backend only, excluding startup and rendering. Warnings are
permission and coverage notices; live systems never give two runs identical
coverage, so these numbers show scale, not a speedup ratio.

### Windows: handle backend vs. Restart Manager

The current Windows folder backend was compared on native GitHub runners with
the parallel Restart Manager backend it replaced
([PR #63](https://github.com/karimz1/open-file-lock-handle/pull/63)). Each
figure is the median of five alternating runs after one warm-up, and both
backends had to report identical fixture results:

| Runner | Fixture | Restart Manager | Handle backend |
| --- | --- | ---: | ---: |
| x64 | 2,048 files, 8 held | 795.589 ms | 310.709 ms |
| x64 | 2,048 files, 128 held | 3,302.953 ms | 315.291 ms |
| x64 | 2,048 files, none held | 215.860 ms | 302.405 ms |
| ARM64 | 2,048 files, 8 held | 989.248 ms | 345.099 ms |
| ARM64 | 2,048 files, 128 held | 5,383.856 ms | 353.413 ms |
| ARM64 | 2,048 files, none held | 213.727 ms | 353.960 ms |

The handle backend is up to 15× faster on busy folders and 87–140 ms slower on
a folder nobody is using, because it always inspects every accessible process
reference. Its cost stays flat as the folder grows. Counters, sample counts,
and p95 timings are in the
[x64](measurements/inspection-windows-x64-2026-10-06.json) and
[ARM64](measurements/inspection-windows-arm64-2026-10-06.json) data from
[this CI run](https://github.com/karimz1/open-file-lock-handle/actions/runs/37474997933).
Design history is in
[issue #62](https://github.com/karimz1/open-file-lock-handle/issues/62#issuecomment-6018347156).

<a id="native-comparison"></a>

## Reproducing the measurements

`inspection_profile` creates 2,048 files in 16 folders and holds 8 (sparse) or
128 (dense) of them open from a separate process; an idle fixture has 2,048
unused files in one folder. Every held file must be found, and a candidate's
users, paths, access, deletion, and sharing evidence must match the baseline
before its timings count.

To compare a change against a baseline, build `scan_bench` on the baseline
revision, then run from the candidate checkout:

```sh
cargo build --release --locked -p oflh-platform --example inspection_profile
./target/release/examples/inspection_profile \
  --baseline /path/to/baseline/target/release/examples/scan_bench
```

Omit `--baseline` for a candidate-only profile. On Windows both paths end in
`.exe`. The harness warms each implementation once, alternates their order, and
times five scans with a fresh backend each time; baseline process startup is
excluded. Five samples are a diagnostic. Repeat on the same idle machine before
claiming a speedup.

Add `--whole-disk --budget-seconds 120` to profile `/` or `C:\`. Each
implementation gets one scan within the budget; a cancelled or failed scan is
reported as such, never as a result. Whole-root runs find extreme costs and
cancellation problems; use the synthetic fixtures for base-vs-candidate
comparisons.

CI runs the comparison against the pull request's base on all six native
targets and uploads JSON counters plus a combined Markdown summary
(`cargo xtask inspection-summary`). The summary rejects missing or duplicate
targets, invalid timings, and unequal fixture coverage. Artifacts contain
aggregate numbers only: no paths, process names, PIDs, accounts, or hostnames.

## What the counters mean

| Counter | Meaning |
| --- | --- |
| `processes` | Process inspection attempts, including processes that could not be opened |
| `resources` | Descriptors, mappings, or modules visited, including those outside the target. Not unique files. |
| `files`, `directories` | Windows fallback work. Zero on Unix and for a healthy Windows handle scan. |
| `resource_queries`, `resource_query_ms` | Restart Manager calls, including registration, retries, and failures. Concurrent calls are summed, so the time can exceed wall time. |
| `resource_workers` | Restart Manager fallback worker count |
| `module_snapshots`, `module_snapshot_ms` | Windows module snapshots and their duration |
| `file_identity_queries` | Windows metadata opens used to compare file identities |
| `native_file_user_queries` | `FileProcessIdsUsingFileInformation` calls after Restart Manager error 6, including buffer retries |

Counters belong to one cancellation token and are read without locking, so a
live read is approximate but never goes backwards. They count work attempted,
not proof of complete coverage. The progress displays in both apps show the
attempted process and resource counts.

<a id="process-reference-windows-folder-inspection"></a>

## Windows folder backend

Folder and drive scans run in a headless helper process:

1. Take one system-wide handle snapshot (`NtQuerySystemInformation` class 64)
   and check its record layout against
   [phnt](https://github.com/winsiderss/phnt/blob/master/ntexapi.h) before
   reading. An unexpected layout fails explicitly.
2. Identify file objects by comparing with a handle the helper opened itself.
3. Spread the owning processes across two workers per logical CPU, at most
   eight. Each worker duplicates handles into owned guards, queries their
   names, and reads data mappings with `VirtualQueryEx` and
   [`GetMappedFileNameW`](https://learn.microsoft.com/en-us/windows/win32/api/psapi/nf-psapi-getmappedfilenamew),
   which also catches files mapped after their handle closed.
4. Check hard-link aliases opened under another name by full file ID
   ([`FindFirstFileNameW`](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-findfirstfilenamew)).
5. Probe matching files for sharing conflicts (see below).
6. Re-check each process's birth identity before publishing results.

Kernel object addresses are never dereferenced or used as cache keys, and
access flags captured in the snapshot are not trusted after a handle slot could
have been reused.

**Helper isolation.** The helper is the same executable started with a hidden
argument. The parent puts it in a job object with kill-on-close before sending
work, and talks to it over a private versioned binary protocol that preserves
UTF-16 exactly (including unpaired surrogates) and rejects malformed or
oversized frames. If the helper does not start within five seconds, or makes no
progress for twenty, the parent stops it, keeps the observations already
received, and reports a warning. A scan that keeps progressing has no time
limit. Cancel stops only the helper, never an inspected process. Library
embedders configure their own helper entry point with
`native_with_inspection_helper`.

**Sharing probe.** Whole-drive diagnostics once stalled on a file with an
oplock. Probes now use `NtCreateFile` with `FILE_COMPLETE_IF_OPLOCKED`, maximum
sharing, and `FILE_OPEN_NO_RECALL`
([Microsoft docs](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/ntifs/nf-ntifs-ntcreatefile)),
close the handle without reading or writing, and count only
`STATUS_SHARING_VIOLATION` as evidence. Other failures stay "unknown" with
their status code. A regression fixture holds an oplock without acknowledging
the break and requires the scan to finish with no false conflict. Other
file-system or filter-driver calls can still block, which is why the helper
stall limit exists.

**Deleted files.** With POSIX-style delete, Windows can drop a file's original
name while a handle stays open. A native fixture checks both cases: if the name
is still visible the scanner must report it, otherwise it must warn that the
folder is unknown rather than guess.

Native regressions for this backend require 160 independent users of one file,
a held file behind 10,000 unused ones, read-only, write-only, and
metadata-only handles, directory handles, mappings after close, outside hard
links, and deleted-name handling. See
[regression coverage](inspection-regressions.md).

## Windows single files and fallback

Single-file targets use Restart Manager, with file-identity checks for loaded
modules. If the helper cannot start, folder scans fall back to Restart Manager
too, with a disclosed 10,000-file cap.

The fallback uses two workers per logical CPU (at most eight), each with up to
two queued batches of 128 files. Batches start at 128 files and double up to
1,024 while results stay empty; an occupied or failed batch drops back to 128
and is split down to single files, because a user of a batch is not necessarily
a user of every file in it. Batches carry across tiny folders to avoid one
expensive registration per folder.
[Resource registration writes to the registry](https://learn.microsoft.com/en-us/windows/win32/api/restartmanager/nf-restartmanager-rmregisterresources),
so fewer, larger batches help in unused areas. A fixed 1,024-file batch made
occupied fixtures 9–36% slower, which is why batches stay small where files are
in use.

**Error 6 recovery.** On the test runners, Restart Manager returns
`ERROR_INVALID_HANDLE` for a file shared by many processes. oflh then calls
`FileProcessIdsUsingFileInformation` for that file, captures process births
before the PID-only query, verifies them afterwards, and labels the rows
`native file user`. Microsoft
[reserves this query for system use](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/wdm/ne-wdm-_file_information_class),
so its use is limited to this recovery path and every failure is reported.

**Developer probe.** `windows_native_probe` measures that query and an
experimental parallel handle scan in isolation. It needs an opt-in feature that
shipped builds never enable:

```sh
cargo run --release --locked -p oflh-platform --example windows_native_probe --features native-query-experiment
```

It measures discovery only (no metadata, mappings, or lock evidence), so its
timings must not be presented as end-to-end speedups. A
[dedicated workflow](../.github/workflows/windows-probe.yml) runs it on Windows
x64 and ARM64.

## macOS process workers and phase profiling

macOS hands each PID to one of two workers per logical CPU (at most eight)
through a shared atomic cursor. Each worker owns its processes, user-name cache,
and partial-coverage count, and all workers finish before results are
published, including after cancellation, an error, or a panic. Concurrent scans
in the same app share one budget of worker permits, so two scans do not double
the pressure on the OS. Birth identities are checked after inspection and again
after enrichment.

The profiler reports `process_workers`, `process_concurrency_slots`,
`process_metadata_ms`, `descriptor_ms`, `mapping_ms`, and `lock_probe_ms`.
Concurrent durations are summed and can exceed wall time. Investigation:
[issue #66](https://github.com/karimz1/open-file-lock-handle/issues/66).

## Desktop search and navigation

The desktop keeps the snapshot and search index in Rust, sends at most 200 rows
per page, and virtualizes the grid. One filtered and sorted index list is
cached per snapshot and shared by paging, selection, and column fitting. The
frontend keeps one page request in flight and replaces any pending request with
the latest, so fast typing cannot queue stale searches.

On a [50,000-row synthetic grid](measurements/desktop-grid-linux-2026-10-05.json)
(500 processes, 100 page requests, local Linux machine), median page retrieval
fell from 39.540 ms to 0.139 ms (p95 40.886 to 0.145 ms). Initial search stayed
at 41–42 ms and indexing at 36–37 ms. Skipping relevance scoring when the user
sorts by another column cut the
[first path-sorted query](measurements/desktop-grid-lazy-score-linux-2026-10-06.json)
from 40.04 ms to 3.57 ms. These measure Rust search and paging, not WebView
rendering.

```sh
cargo run --release --locked -p oflh-desktop --example grid_profile
cargo run --release --locked -p oflh-desktop --example grid_profile -- --baseline /path/to/baseline/grid_profile
```

## Terminal navigation and search

The terminal draws only the visible rows but keeps the full snapshot. Search
fields, lower-case name keys, and PID lookup tables are built on the scanner
thread before results reach the UI. Sorting by name, RAM, CPU, or PID skips
relevance scoring, and metric updates use identity lookups.

[Five alternating local Linux runs](measurements/tui-navigation-linux-2026-10-07.json)
over 50,000 synthetic usages, before and after that change:

| Workload | Before | After |
| --- | ---: | ---: |
| 500 processes: wildcard/name search | 78.792 ms | 11.912 ms |
| 10,000 processes: wildcard/name search | 91.430 ms | 16.832 ms |
| 50,000 lock rows, 500 processes: search | 130.211 ms | 24.563 ms |
| 50,000 lock rows, 10,000 processes: search | 142.908 ms | 30.572 ms |
| 10,000 identity-bound metric updates | 29.000 ms | 0.367 ms |
| 100 lock-table End/draw frames, 10,000 processes | 272.434 ms | 32.854 ms |

Indexing plus initial matching rose slightly (52.248 → 53.752 ms for 500
processes, 35.938 → 39.513 ms for 10,000) because that work moved off the input
thread. Timings use ratatui's `TestBackend` and exclude terminal output and
native scanning.

```sh
cargo run --release --locked -p oflh-tui --features profiling --example navigation_profile
```

CI builds this harness for the base and the candidate on all six targets and
checks that every usage is matched exactly once and the last row is reachable.
