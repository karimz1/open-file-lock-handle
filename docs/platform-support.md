# Platform behavior

[Back to the README](../README.md) · [Terminal guide](terminal-usage.md) · [Desktop guide](desktop-usage.md)

The workflow is shared across platforms, but discovery and lock semantics depend
on the operating system. Results are a snapshot of what the current user can
inspect. Permissions, process exits, and concurrent file activity can limit them.
When a scan has limitations, the footer shows "Results may be incomplete."
Press `?` for the full scan details.

## File discovery and termination

| Platform | Discovery | Normal termination (`k`) |
| --- | --- | --- |
| Linux | `/proc`: file descriptors, CWD, executable, mapped files, deleted-but-open files | `SIGTERM` with `pidfd` identity validation |
| macOS | `libproc`: vnode descriptors, CWD, executable, mapped files | `SIGTERM` after start-time validation |
| Windows | Live file handles and data mappings for folders; Restart Manager/native file-user recovery for individual files; Toolhelp modules and executables | `WM_CLOSE` for process windows |

Actions revalidate PID and process birth identity before signaling. Linux uses
an owned pidfd; Windows force termination uses a validated process handle. macOS
checks start time before signaling, but its APIs leave a narrow exit/PID-reuse race.

Windows console and service processes may require explicit force termination.
Windows folder inspection follows process references rather than walking unused
files. Accessible directory handles and delete-pending handles with resolvable names are included;
working-directory classification is unavailable. POSIX-style unlink on modern Windows can discard the old parent/name while the handle remains live. Such handles cannot be assigned to their former folder safely and produce explicit partial-coverage warnings. Data mappings are inspected even
when the file handle has closed. Their native device paths must have a supported
DOS-drive or UNC translation; mounted-volume-only paths and outside-name hard-link
aliases of closed-handle mappings may be missed. Open-file hard-link aliases are
checked using the held file identity. Permissions, changed handles and unresolved
paths are reported as partial inspection warnings. No process-list cutoff applies.
The native handle ABI is checked; helpers that cannot start fall back to limited
Restart Manager discovery, with its 10,000-file cap explicitly disclosed.
Individual-file inspection retains the existing identity-aware backend.
When Restart Manager returns error 6, the scanner attempts a native file-user
query and labels recovered observations `native file user`. This query is
reserved by Microsoft; unsupported filesystems or failed queries retain explicit
warnings. Each recovered PID must match a process birth captured before the query
and checked again before publication. Neither source proves lock ownership.
On Linux, other mount namespaces may require running `oflh` inside the relevant
container. Elevated privileges can improve visibility but do not remove every
platform limitation.

## Lock evidence

An open file is not necessarily locked. The Locked files view requires additional
evidence:

| Platform | Evidence | Scope |
| --- | --- | --- |
| Linux | Held FLOCK, POSIX, and OFD locks from `/proc/PID/fdinfo` | Subject to permissions, namespaces, and scan timing |
| macOS | POSIX byte-range conflicts queried with `F_GETLK` | First conflicting range per readable file; flock-only locks and additional ranges may be missed |
| Windows | Read, write, or delete sharing conflicts, correlated with observed file users | Reported users are labeled **owner unverified**; byte-range locks are not enumerated |

On Windows, a sharing conflict confirms the file is restricted, but does not
prove which reported process imposed that restriction. Permission-denied errors
alone are never classified as locks. Lock queries do not modify file contents.
Advisory locks do not necessarily prevent ordinary reads or writes.

## Access modes

The **ACCESS** column summarizes the usages matching the current search.
`read/write` means both modes were observed, possibly on different files. `cwd`
means the process uses the folder as its working directory; it does not imply
read or write access. These labels describe observed access modes, not live I/O
activity or proof of a lock.

Read access uses green, write access uses amber, and confirmed locks use muted
red. Text labels carry the meaning without relying on color.

## CPU, memory, and ancestry

All three platforms collect resident memory and up to eight observed ancestors.
Memory is RSS on Unix and working set on Windows.

CPU measures a process's share of total machine capacity over the sampling
interval: 100% means all CPUs. It requires two samples of the same process. A
lightweight metrics sample runs about one second after the initial results;
use `a` for regular updates. Unavailable metrics appear as a dash, for example when
permissions or process exit prevent inspection.

## Ports

Ports are collected through `netstat2` using native OS APIs, without executing
`lsof`, `ss`, or `netstat` and without connecting to services.

| Platform | Socket discovery | Path association |
| --- | --- | --- |
| Linux | Netlink socket diagnostics; procfs for owner PIDs | Existing file, mapping, executable, and working-directory observations |
| macOS | `libproc` socket descriptors | Existing file, mapping, executable, and working-directory observations |
| Windows | IP Helper TCP/UDP owner tables | Existing Restart Manager, executable, and module observations; no working-directory inspection |

TCP results include LISTEN sockets only, not established connections or TIME_WAIT
entries. UDP results include bound local sockets, including client sockets;
BOUND does not mean a listening TCP-style service. IPv4 and IPv6 are enumerated
separately, so failure in one family does not discard the other's results.

Ownership is joined only when the same PID and birth identity were observed
before and after socket discovery. Missing, inaccessible, new, or changed owners
appear as unavailable, with no actionable PID. Permissions can hide entire
sockets on macOS. Linux discovery is limited to the current network namespace;
run inside a container to inspect its namespace. Docker forwarding metadata is
not queried. IPv6 interface scope identifiers are not exposed by the socket
collector; addresses are informational, not ready-to-use connection commands.

Path and socket scans are sequential snapshots. A process starting or changing
its file references between them may not appear in THIS PATH until a refresh.
Folder association remains subject to the file-discovery limits above. No result
proves firewall access or reachability from another machine.
