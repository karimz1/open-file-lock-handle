# Measuring inspection performance

Linux, macOS and the Windows folder backend enumerate process references instead of every unused file below a folder. Windows uses an owned headless helper for live disk handles and data mappings, alongside existing executable/module discovery. Healthy folder inspection has no 10,000-file cap or process-count cutoff. Permissions, native-query failures and unsupported paths remain explicit limitations; observations do not prove lock ownership. Individual files retain Restart Manager and the identity-aware native recovery backend. An embedding binary without a helper uses an explicitly limited Restart Manager folder fallback.

## Native comparison

`inspection_profile` is a developer example; its helpers and dependencies are excluded from `cargo build --release --locked --bin oflh`. It creates 2,048 synthetic files in 16 folders and holds 8 files for the sparse fixture or 128 for the dense fixture in a separate process. The idle fixture has 2,048 unused files in one directory. Each held file must be discovered. The candidate must retain the same fixture users, paths, access, deletion and sharing-conflict evidence before timings are accepted. Windows `open`, `restart manager` and `native file user` sources are compared as file-user associations; their different evidence sources remain distinct in application rows. Other relations remain exact. This comparison does not certify arbitrary live-system coverage; independent native regressions cover additional handle and mapping cases.

Build `scan_bench` on the baseline revision, then run from the candidate checkout:

```sh
cargo build --release --locked -p oflh-platform --example inspection_profile
./target/release/examples/inspection_profile --baseline /path/to/baseline/target/release/examples/scan_bench
```

On Windows, both executable paths end in `.exe`. Omit `--baseline` for a candidate-only profile. The harness warms each implementation once, alternates execution order and records five timed scans with a fresh backend each time. Baseline subprocess startup is excluded from the timing. Five samples are a diagnostic, not a reliable estimate of long-tail latency. Run repeated comparisons on the same otherwise idle machine before making performance claims.

CI runs the comparison on native Linux, macOS and Windows, on both x86-64 and ARM64. Every run uploads JSON counters and a combined Markdown comparison. The summary refuses missing/duplicate targets, invalid timings and unequal fixture coverage. Different runner hardware, permissions and OS evidence mean the table cannot rank operating systems by speed. The artifacts contain aggregate synthetic coverage and timing data, without native paths, process names, PIDs, accounts or hostnames.

## What the counters mean

- `processes`: process inspection attempts, including unavailable processes.
- `resources`: descriptors, mappings or modules visited, including references outside the target. These are not unique files.
- `files` and `directories`: matching Windows file paths and fallback resource-inspection work. Healthy handle inspection does not walk directories; Unix backends report zero here.
- `resource_queries` and `resource_query_ms`: Restart Manager calls, including resource registration, retries and failed calls.
- `module_snapshots` and `module_snapshot_ms`: Windows module snapshot attempts and their duration.
- `file_identity_queries`: Windows metadata opens used to compare file identity, including the target probe.
- `native_file_user_queries`: native compatibility queries after Restart Manager error 6, including buffer retries.

Counters belong to one cancellation token. Clones share the same progress; a new inspection starts with empty counters. Reads are approximate and monotonic, without a lock on the worker. Native calls already running can finish before cancellation is observed. Counts are work attempted, not proof of complete coverage.

## Windows changes and remaining costs

The limited Restart Manager fallback uses a bounded pool of two workers per logical CPU, capped at eight, with at most two queued 128-file batches per worker. Enumeration and resource queries overlap. Each worker owns its process metadata cache and native sessions; no query runs under the queue lock. Every worker is joined before results are published. Cancellation stops dispatch and queued work, while a native call already running can finish later. `resource_workers` records the actual worker count; `resource_query_ms` sums overlapping calls and can exceed scan elapsed time. This is an algorithm experiment whose native timing artifacts determine whether it should be retained.

Parallel directory batches stay at 128 to avoid excessive subdivisions in occupied areas. Single-file inspection and the serial regression helper retain the following adaptive policy.

The scanner starts with 128 files and doubles the batch size up to 1,024 after an empty result. Occupied or unavailable results return to 128. Growth resets at directory boundaries, while small tails carry across directories to avoid one expensive registration per tiny folder. Empty batches need one query; occupied batches still split down to individual files because a batch user is not evidence that it uses every file. Self-only batches stop immediately because the scanner excludes itself from results. Repeated module paths reuse file-identity probes within one scan; the next scan starts a fresh cache. Process birth identities are still checked before publication and before actions.

[Microsoft documents expensive registry writes in resource registration](https://learn.microsoft.com/en-us/windows/win32/api/restartmanager/nf-restartmanager-rmregisterresources). Adaptive grouping reduces registration count in unused areas, but occupied areas still require expensive queries. Initial native measurements showed fixed 1,024-file batches regressed Windows sparse/dense cases by about 9–36%; the scanner therefore keeps small batches in occupied regions rather than applying a global increase. System-wide process references remove that dependence on unused disk files. The implementation and coverage gates are described below; no whole-drive speedup is inferred from different live-system coverage.

Local Linux measurements are recorded in [the synthetic profile](measurements/inspection-linux-2026-10-05.json). Windows and macOS performance claims require their native CI artifacts. This fixture does not reproduce every C-drive permission, network, antivirus or filesystem condition.

## Windows completeness recovery and discovery experiments

The native 160-process regression found that Restart Manager returns error 6 (`ERROR_INVALID_HANDLE`) for a file shared by many users on the tested Windows runners. Successful resource queries keep their existing evidence. Failed groups are split down to individual files, where the scanner attempts `FileProcessIdsUsingFileInformation`. It captures process births before this PID-only query and verifies them before publication. Recovered rows say `native file user`; sharing-conflict evidence remains separate and owner uncertainty is preserved. The original error and use of this reserved query remain visible in warnings. Unsupported queries produce explicit partial-result warnings, never a shortened list presented as complete.

`windows_native_probe` tests the native file-user query separately from the production backend. It requires the opt-in `native-query-experiment` feature, which the distributed CLI and desktop do not enable. Its dedicated workflow runs on Windows x86-64 and ARM64 and uploads aggregate synthetic results. Run it on a native Windows development machine with:

```sh
cargo run --release --locked -p oflh-platform --example windows_native_probe --features native-query-experiment
```

The probe measures discovery of 128 held files among 2,048 files, then checks 160 processes sharing one file against both the direct query and the current backend. Discovery timings exclude process metadata, birth validation, mappings and lock evidence; they must not be presented as complete inspection speedups. It reads file metadata, not file contents. Variable-length native results are checked against the SDK layout and returned byte count; an exceeded buffer budget is an error, never a truncated list.

[Microsoft reserves `FileProcessIdsUsingFileInformation` for system use](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/wdm/ne-wdm-_file_information_class). Its production use is limited to compatibility recovery after Restart Manager error 6, with explicit warnings and failures. It remains the individual-file/fallback compatibility path; the handle backend supplies uncapped folder discovery. A strategy for huge folders still needs to avoid walking every unused file. Track these decisions in [the Windows algorithm investigation](https://github.com/karimz1/open-file-lock-handle/issues/62).

The same probe also experiments with one system-wide handle snapshot and parallel inspection of open disk handles. It derives the file object type from its own live metadata handle, verifies the [phnt native record layout](https://github.com/winsiderss/phnt/blob/master/ntexapi.h), duplicates handles into owned guards and rejects observations after the source process exits or its birth identity changes. It does not cache results by kernel object address. Permission failures and handles that disappear are counted explicitly. This scope omits mappings whose file handles have closed, modules and lock evidence; it is not a complete replacement backend.

Native handle-path queries can block, so this experiment runs in a separate helper process with a 30-second budget. A budget outcome is not a completed scan. The parent terminates only its own experimental helper, never an inspected application. [Microsoft warns that `NtQuerySystemInformation` can change](https://learn.microsoft.com/en-us/windows/win32/api/winternl/nf-winternl-ntquerysysteminformation); the snapshot ABI and buffer extents are checked before any record is read. The developer probe measures a narrower discovery scope than the production folder backend and must not be presented as an end-to-end application speedup.

## Whole-root diagnostics

Add `--whole-disk --budget-seconds 120` to profile `/` on Linux/macOS or `C:\` on Windows. CI includes this extreme case alongside the stable fixtures. Each implementation gets one scan with a two-minute budget. The candidate requests cooperative cancellation; an already running native call may finish later. The baseline process is stopped if it exceeds the budget. Cancelled/failed scans are reported as such, with aggregate candidate work counters; they are never counted as completed scans or improvements.

These are backend root inspections, not equivalent traversal of every disk file. Unix enumerates visible process references. Windows follows accessible handles, data mappings, executables and modules; its fallback retains a disclosed 10,000-file cap. Both implementations retain permission warnings. Live processes also change between runs, so the root diagnostic does not claim equal inspection coverage. Use the stable synthetic fixtures for base/candidate speed comparisons and the root diagnostic to find extreme costs or cancellation limits.

## Process-reference Windows folder inspection

The production folder helper takes one checked class-64 handle snapshot, identifies
file objects using its own live metadata handle, and distributes source processes
across two workers per logical CPU, capped at eight. Workers own process handles
and duplicated file handles. No kernel pointer is dereferenced or used as a path
cache; captured access flags are not trusted after a handle slot can change.
Actual disk handles are named, directory/delete-pending metadata is queried (original deleted names may be unavailable), and
outside-opened hard-link aliases are verified using full file IDs. Data mappings
are found with `VirtualQueryEx` and `GetMappedFileNameW`, including mappings after
the file handle closes. Loaded modules and executables keep their existing scan.

[Microsoft documents mapping-name lookup](https://learn.microsoft.com/en-us/windows/win32/api/psapi/nf-psapi-getmappedfilenamew)
and [hard-link name enumeration](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-findfirstfilenamew).
The checked, undocumented handle layout follows
[phnt's native declarations](https://github.com/winsiderss/phnt/blob/master/ntexapi.h);
an unsupported layout fails explicitly. The process birth is captured from owned
handles and checked again before publication. Sharing probes run only on matching
paths inside the helper; reported users remain unverified lock owners.

The private versioned binary protocol preserves UTF-16 (including unpaired
surrogates), rejects malformed or oversized frames, and uses bounded queues.
The parent assigns its helper to a job with kill-on-close before submitting work.
Cancel stops only that owned helper and joins its pipe threads; inspected processes
are never stopped. Five-second startup and twenty-second stalled-work limits
prevent blocked native queries from stranding the UI. Repeated unchanged heartbeats
do not reset the stall limit. A progressing scan has no total-time cutoff.
Incomplete helper work keeps completed observations with explicit warnings.

Native regressions require all 160 independent C users, distinct file users,
a held file beyond 10,000 unused entries, directory references, available deleted names or explicit name-loss warnings, outside
hard links and data mappings after file close. The synthetic timing comparison
must also preserve file-user associations and sharing evidence. Remaining native
limits are in [platform support](platform-support.md).

`native_with_inspection_helper` lets library embedders configure their own headless
entry point. The shipped CLI, desktop and profiling harnesses dispatch the hidden
helper argument before terminal or WebView startup. Missing helpers fall back
explicitly; no fixture or developer benchmark is linked into the application.
New aggregate counters separate handle snapshot duration, handle names, queried
memory regions and mapped-file names from Restart Manager calls.

Modern Windows POSIX-style unlink can make both final-path modes and file-name
information lose the original name. A native fixture independently checks those
APIs: if the original name remains visible, the scanner must retain the deleted
reference; otherwise it must disclose the unknown original folder and must not
guess an association. This is an OS evidence limitation, not a process-count cap.
