# Measuring inspection performance

Linux and macOS enumerate process references rather than every file below a folder. Windows also walks the selected folder and asks Restart Manager about resource users. A large Windows folder can therefore cost much more than a Unix root inspection. Windows directory resource discovery remains capped at 10,000 files and reports partial coverage. Executable and module discovery is separate from that cap. Open references and Restart Manager users do not prove lock ownership.

## Native comparison

`inspection_profile` is a developer example; its helpers and dependencies are excluded from `cargo build --release --locked --bin oflh`. It creates 2,048 synthetic files in 16 folders and holds 8 files for the sparse fixture or 128 for the dense fixture in a separate process. The idle fixture has 2,048 unused files in one directory. Each held file must be discovered. The candidate must retain exactly the baseline's fixture observations, including relation, access, deletion and lock evidence, before timings are accepted.

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
- `files` and `directories`: Windows resource-inspection work. Unix backends do not walk the target tree and report zero here.
- `resource_queries` and `resource_query_ms`: Restart Manager calls, including resource registration, retries and failed calls.
- `module_snapshots` and `module_snapshot_ms`: Windows module snapshot attempts and their duration.
- `file_identity_queries`: Windows metadata opens used to compare file identity, including the target probe.

Counters belong to one cancellation token. Clones share the same progress; a new inspection starts with empty counters. Reads are approximate and monotonic, without a lock on the worker. Native calls already running can finish before cancellation is observed. Counts are work attempted, not proof of complete coverage.

## Windows changes and remaining costs

Directory resource queries now use a bounded pool of two workers per logical CPU, capped at eight, with at most two queued 128-file batches per worker. Enumeration and resource queries overlap. Each worker owns its process metadata cache and native sessions; no query runs under the queue lock. Every worker is joined before results are published. Cancellation stops dispatch and queued work, while a native call already running can finish later. `resource_workers` records the actual worker count; `resource_query_ms` sums overlapping calls and can exceed scan elapsed time. This is an algorithm experiment whose native timing artifacts determine whether it should be retained.

Parallel directory batches stay at 128 to avoid excessive subdivisions in occupied areas. Single-file inspection and the serial regression helper retain the following adaptive policy.

The scanner starts with 128 files and doubles the batch size up to 1,024 after an empty result. Occupied or unavailable results return to 128. Growth resets at directory boundaries, while small tails carry across directories to avoid one expensive registration per tiny folder. Empty batches need one query; occupied batches still split down to individual files because a batch user is not evidence that it uses every file. Self-only batches stop immediately because the scanner excludes itself from results. Repeated module paths reuse file-identity probes within one scan; the next scan starts a fresh cache. Process birth identities are still checked before publication and before actions.

[Microsoft documents expensive registry writes in resource registration](https://learn.microsoft.com/en-us/windows/win32/api/restartmanager/nf-restartmanager-rmregisterresources). Adaptive grouping reduces registration count in unused areas, but occupied areas still require expensive queries. Initial native measurements showed fixed 1,024-file batches regressed Windows sparse/dense cases by about 9–36%; the scanner therefore keeps small batches in occupied regions rather than applying a global increase. Inspect a narrower folder for complete Windows resource coverage. Changing to system-wide handle enumeration would require another backend design with verified ABI layouts, safe handling of blocking name queries and equivalent native coverage; it is not justified solely by faster timings on different hardware.

Local Linux measurements are recorded in [the synthetic profile](measurements/inspection-linux-2026-10-05.json). Windows and macOS performance claims require their native CI artifacts. This fixture does not reproduce every C-drive permission, network, antivirus or filesystem condition.

## Direct Windows discovery experiments

`windows_native_probe` tests the native file-user query separately from the production backend. It requires the opt-in `native-query-experiment` feature, which the distributed CLI and desktop do not enable. Its dedicated workflow runs on Windows x86-64 and ARM64 and uploads aggregate synthetic results. Run it on a native Windows development machine with:

```sh
cargo run --release --locked -p oflh-platform --example windows_native_probe --features native-query-experiment
```

The probe measures discovery of 128 held files among 2,048 files, then checks 160 processes sharing one file against both the direct query and the current backend. Discovery timings exclude process metadata, birth validation, mappings and lock evidence; they must not be presented as complete inspection speedups. It reads file metadata, not file contents. Variable-length native results are checked against the SDK layout and returned byte count; an exceeded buffer budget is an error, never a truncated list.

[Microsoft reserves `FileProcessIdsUsingFileInformation` for system use](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/wdm/ne-wdm-_file_information_class). A successful experiment does not establish a supported API contract or justify replacing the backend. A reliable design still needs explicit failure handling, native coverage tests, process birth validation and a strategy for huge folders that avoids walking every unused file. Track these decisions in [the Windows algorithm investigation](https://github.com/karimz1/open-file-lock-handle/issues/62).

## Whole-root diagnostics

Add `--whole-disk --budget-seconds 120` to profile `/` on Linux/macOS or `C:\` on Windows. CI includes this extreme case alongside the stable fixtures. Each implementation gets one scan with a two-minute budget. The candidate requests cooperative cancellation; an already running native call may finish later. The baseline process is stopped if it exceeds the budget. Cancelled/failed scans are reported as such, with aggregate candidate work counters; they are never counted as completed scans or improvements.

These are backend root inspections, not equivalent traversal of every disk file. Unix enumerates visible process references. Windows retains the 10,000-file resource cap and permission warnings. Live processes also change between runs, so the root diagnostic does not claim equal inspection coverage. Use the stable synthetic fixtures for base/candidate speed comparisons and the root diagnostic to find extreme costs or cancellation limits.
