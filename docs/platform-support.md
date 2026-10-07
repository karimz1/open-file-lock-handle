# Platform support

oflh behaves the same way on every platform, but what it can detect depends on
what each operating system exposes. This page explains how each backend works
and where its blind spots are, so you can tell a real "nothing is using this"
from "oflh could not see it".

Supported targets: Windows, Linux, and macOS, each on x86-64 and ARM64. ARM32 is
not supported.

- [General rules](#general-rules)
- [How files are found](#file-discovery-and-termination)
- [Lock evidence](#lock-evidence)
- [Access modes](#access-modes)
- [CPU, memory, and ancestry](#cpu-memory-and-ancestry)
- [Ports](#ports)
- [Stopping processes](#stopping-processes)

## General rules

- Every result is a snapshot. Processes can open or close files between the
  scan and the moment you read it.
- oflh sees what the current account can inspect. Running elevated (`sudo`,
  an Administrator terminal) shows more processes, but does not remove every
  OS restriction.
- When anything was skipped, the footer says *Results may be incomplete*. Press
  `?` in the terminal app, or open **coverage notices** in the desktop app, for
  the reasons.
- oflh never reads file contents and never modifies the files it inspects.

<a id="file-discovery-and-termination"></a>

## How files are found

On every platform, oflh enumerates the files that running processes reference
and keeps those under the target. It does not walk the directory tree, so a
folder with a million unused files costs no more than an empty one.

| | Linux | macOS | Windows |
| --- | --- | --- | --- |
| Source | `/proc/<pid>` | `libproc` | Native handle snapshot, Restart Manager, Toolhelp |
| Open files | `fd/` | vnode descriptors | Open disk handles |
| Working directory | `cwd` | yes | Not available |
| Executable | `exe` | yes | yes |
| Mapped files, DLLs, shared libraries | `maps` | yes | Data mappings and loaded modules |
| Deleted but still open | yes | — | When the name is still resolvable |

### Linux

oflh reads `/proc/<pid>/fd`, `cwd`, `exe`, and `maps`, and recognizes files
that were deleted while still open. Processes in another mount namespace
(containers) report paths relative to that namespace; run oflh inside the
container to inspect it.

### macOS

oflh uses `libproc` to list vnode descriptors, working directory, executable,
and mapped files. Processes are inspected in parallel, at most two workers per
logical CPU and no more than eight in total.

### Windows

Folder and drive scans use a helper process that takes one system-wide handle
snapshot, resolves the names of open disk handles, and lists each process's
data mappings with `VirtualQueryEx` and `GetMappedFileNameW`. Loaded modules and
executables come from Toolhelp. There is no file-count or process-count limit.

Known gaps:

- Working directories cannot be read, so a shell sitting in a folder is not
  reported as using it.
- A file deleted with POSIX semantics (the default on recent Windows) can lose
  its original name while the handle stays open. oflh then cannot place it in a
  folder and reports a partial-coverage warning instead of guessing.
- Mappings whose native path has no drive-letter or UNC equivalent (for
  example, volumes mounted only as a folder) may be missed, as may other
  hard-link names of a mapped file whose handle is already closed.
- If the helper cannot start, oflh falls back to Restart Manager, which is
  limited to 10,000 files per scan. The warning says so.

Single-file targets use Restart Manager, which reports which processes use the
file. If Restart Manager fails with error 6 (`ERROR_INVALID_HANDLE`, seen with
files shared by many processes), oflh asks the file system directly using
`FileProcessIdsUsingFileInformation` and labels those rows `native file user`.
Microsoft documents this query as reserved for system use, so failures are
reported rather than hidden. Neither source proves which process holds a lock.

## Lock evidence

An open file is not necessarily a locked file. The **Locked files** view (and
the desktop's **Lock evidence only** filter) requires extra evidence:

| Platform | Evidence | Limits |
| --- | --- | --- |
| Linux | `flock`, POSIX, and OFD locks held, from `/proc/<pid>/fdinfo` | Subject to permissions, namespaces, and timing |
| macOS | POSIX byte-range conflicts, queried with `F_GETLK` | Only the first conflicting range per readable file; `flock`-only locks can be missed |
| Windows | A read, write, or delete sharing violation when probing the file | Byte-range locks are not enumerated; see below |

On Windows, a sharing violation proves the file is restricted, but not which of
the processes using it imposed the restriction. Those rows are labeled
**owner unverified**. A permission-denied error alone is never treated as a
lock. The Windows probe opens the file with maximum sharing and without
triggering offline-file recall, then closes it without reading or writing.

Advisory locks on Linux and macOS do not stop other programs from reading or
writing unless they also check the lock.

## Access modes

The **ACCESS** column summarizes how the matching files were opened:

| Label | Meaning |
| --- | --- |
| `read`, `write`, `read/write` | Open mode of the descriptor or handle. `read/write` can also mean different files were opened in different modes. |
| `execute` | Executable image |
| `directory` | A directory, including a working directory. Says nothing about read or write access. |
| `mapped` | Memory mapping with unknown access flags |
| `reference` | Reference-only descriptor, such as Linux `O_PATH` |
| `unknown` | Mode could not be determined |

These labels describe how a file was opened, not whether it is being read or
written right now. Color is only a hint (green read, amber write, muted red
lock); the text label always carries the meaning.

## CPU, memory, and ancestry

All platforms report resident memory (RSS on Linux and macOS, working set on
Windows) and up to eight ancestors per process.

CPU is the process's share of the whole machine over the sampling interval:
100% means every core is busy. It needs two samples, so it appears about a
second after the first results. A dash means the value could not be measured,
for example because of permissions or because the process exited. An
unmeasured value is not zero.

## Ports

Ports are read from the OS socket tables through the `netstat2` crate. oflh does
not run `lsof`, `ss`, or `netstat` and never connects to a port.

| Platform | Sockets from | Owner from |
| --- | --- | --- |
| Linux | Netlink socket diagnostics | `/proc` |
| macOS | `libproc` socket descriptors | `libproc` |
| Windows | IP Helper TCP and UDP owner tables | IP Helper |

- TCP: listening sockets only. Established connections and `TIME_WAIT` entries
  are not listed.
- UDP: every bound local socket, including client sockets. `BOUND` does not
  mean a server is listening.
- IPv4 and IPv6 are queried separately; a failure in one does not hide the
  other. IPv6 scope IDs are not shown.
- An owner is shown only if the same process (PID and start time) was seen both
  before and after reading the socket table. Otherwise the row says *owner
  unavailable* and cannot be acted on.
- Linux only sees the current network namespace. Docker port forwarding is not
  resolved to the container process.
- On macOS, permissions can hide sockets entirely.

**THIS PATH** (terminal) and **Target processes only** (desktop) link ports to a
path by checking which socket owners also appear in the file results. On
Windows that link can only come from open files, modules, and executables, not
the working directory. The two scans run one after the other, so a process that
changes in between may only appear after a refresh. No port result says
anything about firewall rules or reachability from another machine.

## Stopping processes

| Platform | Terminate | Force | Identity check |
| --- | --- | --- | --- |
| Linux | `SIGTERM` | `SIGKILL` | Owned `pidfd`, so the signal cannot reach a reused PID |
| macOS | `SIGTERM` | `SIGKILL` | Start time re-checked just before signaling |
| Windows | `WM_CLOSE` to the process's windows | `TerminateProcess` | Validated process handle |

On Linux, process actions need kernel 5.3 or newer for `pidfd_open`; older
kernels report an error instead of falling back to an unchecked signal.

macOS has no `pidfd` equivalent, so a very narrow window remains in which a
process could exit and its PID be reused between the check and the signal.

Windows console programs and services have no window to close and usually need
**Force**.

oflh refuses to act on PID 0, PID 1 (`init`/`launchd`), itself, and any process
whose start time it could not read. Administrator rights do not change that,
and the OS may still refuse processes it protects.
