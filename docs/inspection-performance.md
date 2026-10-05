# Measuring inspection performance

Linux and macOS enumerate process references rather than every file below a folder. Windows also walks the selected folder and asks Restart Manager about resource users. A large Windows folder can therefore cost much more than a Unix root inspection. Windows directory resource discovery remains capped at 10,000 files and reports partial coverage. Executable and module discovery is separate from that cap. Open references and Restart Manager users do not prove lock ownership.

## Native comparison

`inspection_profile` is a developer example; its helpers and dependencies are excluded from `cargo build --release --locked --bin oflh`. It creates 2,048 synthetic files in 16 folders and holds 8 files for the sparse fixture or 128 for the dense fixture in a separate process. Each held file must be discovered. The candidate must retain exactly the baseline's fixture observations, including relation, access, deletion and lock evidence, before timings are accepted.

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

The scanner groups up to 1,024 files instead of 128. Empty batches need one query; occupied batches still split down to individual files because a batch user is not evidence that it uses every file. Self-only batches stop immediately because the scanner excludes itself from results. Repeated module paths reuse file-identity probes within one scan; the next scan starts a fresh cache. Process birth identities are still checked before publication and before actions.

[Microsoft documents expensive registry writes in resource registration](https://learn.microsoft.com/en-us/windows/win32/api/restartmanager/nf-restartmanager-rmregisterresources). The larger grouping reduces registration count in unused areas, but occupied areas still require expensive queries. Inspect a narrower folder for complete Windows resource coverage. Changing to system-wide handle enumeration would require another backend design with verified ABI layouts, safe handling of blocking name queries and equivalent native coverage; it is not justified solely by faster timings on different hardware.

Local Linux measurements are recorded in [the synthetic profile](measurements/inspection-linux-2026-10-05.json). Windows and macOS performance claims require their native CI artifacts. This fixture does not reproduce every C-drive permission, network, antivirus or filesystem condition.
