# Terminal guide

`oflh` is an interactive terminal app. It shows which processes use a file,
folder, or local port, lets you search and drill into the results, and can ask
a process to exit. For installation, see the [README](../README.md#install).

- [Usage](#usage)
- [Reading the results](#file-inspection)
- [Search](#search)
- [Ports](#ports)
- [Process actions](#process-actions)
- [Refresh and cancel](#refresh-and-cancel)
- [Keyboard reference](#keyboard-reference)
- [Languages](#terminal-languages)
- [Release notices](#release-notices)
- [Terminal requirements](#terminal-support)

## Usage

```text
oflh [OPTIONS] [PATH]
```

| Command | What it shows |
| --- | --- |
| `oflh` | Processes using anything under the current directory |
| `oflh ./build` | Processes using `./build` or anything below it |
| `oflh ./build/plugin.dll` | Processes using that one file |
| `oflh /` or `oflh 'C:\'` | Processes using anything on the filesystem or drive |
| `oflh --ports` | All local TCP listeners and bound UDP sockets |
| `oflh --port 3000` | Only bindings on local port 3000 |
| `oflh --ports ./project` | Ports owned by processes that use `./project` |
| `oflh --port 5040 ./project` | The same, filtered to port 5040 |

| Option | Effect |
| --- | --- |
| `--ports` | Start in the Ports tab |
| `--port PORT` | Start in the Ports tab filtered to one port (1–65535) |
| `--language en\|de\|zh\|system` | Interface language; `system` (default) follows the locale |
| `--no-update-check` | Never contact the network to check for releases |
| `--version` | Print the version and build provenance |
| `--help`, `-h` | Print help in the selected language |

Put options before `PATH`; anything after the path is treated as a second path
and rejected. Use `--` to pass a path that starts with a dash:
`oflh -- --weird-name`. Quote paths that contain spaces.

oflh needs an interactive terminal. When stdin or stdout is redirected it exits
with status 1 and the message `an interactive terminal is required`. Invalid
options exit with status 2.

<a id="file-inspection"></a>

## Reading the results

The header shows the inspected target. A folder target includes everything
below it. There are three tabs:

**1 Processes** groups observations by process: PID, name, user, CPU, memory,
access mode, port count, and the best-matching path (`+N` means N more matching
paths).

**2 Locked files** lists only observations with lock or sharing-conflict
evidence. See [platform support](platform-support.md#lock-evidence) for what
counts as evidence on each OS.

**3 Ports** lists local port bindings; see [Ports](#ports).

Press `Enter` on a process to open its details: every matching file with its
relation and access mode, plus executable, working directory, parent, CPU,
memory, and ports. In details, `l` limits the list to lock evidence and `←`/`→`
scroll a long path.

```text
  oflh  /  process details
  dotnet   PID 424242 · alice
  EXE /usr/bin/dotnet
  CWD /build
  PARENT parent (424241)
  CPU 2.4% machine · RAM 31.0 MiB RSS
  ALL USAGES · 3 of 3 usages · 1 locked files   1 / 3
  FILE                          RELATION   ACCESS       DIRECTORY
  FileLockExampleCli.dll        locked     read/write   /build
  FileLockExampleCli.deps.json  open       unknown      /build
  other.dll                     open       unknown      /build
  SELECTED PATH · locked · read/write
  /build/FileLockExampleCli.dll · POSIX WRITE
```

The **RELATION** column says how the process references the file:

| Relation | Meaning |
| --- | --- |
| `open` | Open file descriptor or handle |
| `cwd` | The process's working directory (Linux and macOS) |
| `executable` | The process's own executable |
| `mapped` | Memory-mapped file or loaded module (DLL, shared library) |
| `locked` | Open, with lock or sharing-conflict evidence |
| `restart manager` | Windows Restart Manager reports this process as a user of the file |
| `native file user` | Windows file-user query reports this process as a user of the file |

The last two do not prove that process holds a lock. **ACCESS** shows the
observed mode: `read`, `write`, `read/write`, `execute`, `directory`, `mapped`,
`reference`, or `unknown`. It describes how the file was opened, not live I/O.

The side panel (toggle with `i`) appears on wide terminals and shows the
selected process's parent tree, metrics, ports, and executable. CPU is the share
of the whole machine (100% = all cores) over the last sample; RAM is RSS on Unix
and working set on Windows. A dash means the value could not be measured.

When a scan could not see everything, for example because of permissions, the
footer says *Results may be incomplete*. Press `?` for the details.

## Search

Press `/`, type, then `Enter` to keep the filter or `Esc` to discard it. The
list updates as you type. Search is case-insensitive and applies to process
names, PIDs, paths, and the relation/access labels above.

| Query | Matches |
| --- | --- |
| `dll` | Any name or path containing `dll` |
| `MIMJWT` | Initials of words: `Microsoft.IdentityModel.JsonWebTokens.dll` |
| `FLEC.` | Initials followed by a dot: `FileLockExampleCli.dll` |
| `FLEC*.json` | Initials, anything, then `.json`: `FileLockExampleCli.deps.json` |
| `micro*dll` | `micro`, then anything (including `/`), then `dll` |
| `*.dll` | Any field containing `.dll`, including `plugin.dll.backup` |
| `node mapped` | Both terms must match |
| `pid:424242` | That process ID only |

Initials match the starts of words; scattered letters do not match. Punctuation
is literal. Matches in a file or process name rank above matches that occur
only in a folder name, unless you sort by another column (`n` name, `p` PID,
`m` RAM, `c` CPU).

File-related terms carry over into process details, so the details view opens
already filtered. To see every file a process uses, clear the detail search with
`/`, `Ctrl+U`, `Enter`. Search and filters stay active across refreshes.

## Ports

Press `3` or start with `oflh --ports`. Each row is one address, protocol, and
port; IPv4 and IPv6 are listed separately.

```text
  5 bindings · ALL PORTS · s scope
    PORT   PROTO PID      PROCESS           ADDRESS     STATE   THIS PATH
    3000   TCP   424242   dotnet            127.0.0.1   LISTEN  yes
    3000   TCP   424242   dotnet            ::1         LISTEN  yes
    3000   TCP   424243   other-project     127.0.0.1   LISTEN  —
    5300   UDP   424242   dotnet            127.0.0.1   BOUND   yes
    9000   TCP   —        owner unavailable 0.0.0.0     LISTEN  —
```

TCP rows are listening sockets only, not established connections. UDP rows are
bound sockets, including client sockets, so `BOUND` does not mean a server is
listening. *owner unavailable* means oflh could not confirm which process owns
the socket; such rows cannot be terminated.

**Scope.** Without a path, the tab shows all ports. With a path
(`oflh --ports ./project`) it starts in **THIS PATH**, which shows only ports of
processes that also use that path. Press `s` to switch between the two. THIS
PATH does not prove the project created the socket, and on Windows some dev
servers only appear under ALL PORTS because their working directory cannot be
read.

**Port search** uses its own query, separate from file search:

| Query | Matches |
| --- | --- |
| `50` | Port numbers containing 50, such as 5040 |
| `port:3000` | Exactly port 3000 (not 13000, not a PID) |
| `tcp port:3000` | TCP listeners on 3000 |
| `udp`, `ipv6`, `127.0.0.1` | Protocol, address family, or address |
| `node port:3000` | Process text and exact port must both match |

**From a port to its files.** `Enter` opens the owner's port details, `f` shows
the files it uses, `p` returns to its ports, and `Esc` goes back. If you started
without a path, opening an owner re-targets the file scan to that process's
working directory (or its executable's folder); the header shows which folder
was chosen. If you gave a path, the target stays fixed.

## Process actions

The usual way to free a file or port is to close the program that holds it.
oflh can do that for you:

| Key | Action |
| --- | --- |
| `Space` | Select or deselect the current process |
| `Ctrl+A` | Select or deselect all visible processes |
| `k` | Request normal termination of the selection, or the current process |
| `x` | Force-kill the selection, or the current process |
| `K` / `X` | Same, but with no selection they target **all filtered processes** |

Normal termination sends `SIGTERM` on Linux and macOS and a window-close
request (`WM_CLOSE`) on Windows. Windows console and service processes have no
window and may need `x`. Normal termination never escalates to force on its own.

Every action opens a confirmation that lists all targets, including selected
processes hidden by the current filter, with **Cancel** preselected:

```text
  FORCE KILL 1 processes?
  Immediate termination: no cleanup. Unsaved work may be lost.

  Affected processes (including selections hidden by filters):
    424242    dotnet

  ▶ Cancel      Force kill
```

Before sending anything, oflh checks that each PID still belongs to the same
process (PID plus start time). Protected processes and parents that could not
be identified cannot be stopped. A successful request triggers a refresh, but
does not guarantee the process has exited.

### Act on a parent process

A worker process often belongs to a larger app. Press `Tab` or `→` to focus the
parent tree in the side panel. The current process is highlighted; `↑` moves
toward its parents and `↓` back. In the tree, `k` and `x` apply only to the
highlighted node and ignore the main selection. `Esc`, `←`, or `Tab` leaves the
tree.

The tree keeps the processes it captured across refreshes. Stopping a parent
may close its application and affect its children, but oflh does not
recursively kill the tree. Save your work first.

## Refresh and cancel

| Key | Action |
| --- | --- |
| `r` or `F5` | Rescan now |
| `a` | Toggle automatic rescans, five seconds after each scan finishes |
| `z` | Cancel the running scan |

While a scan runs, the previous results stay usable and the progress line shows
elapsed time and how many processes and resources have been visited (there is no
total, so no percentage). Further refresh requests are ignored until it
finishes. Cancellation is cooperative: an OS call already in progress finishes
first. Automatic refresh pauses while you edit a search, read help or a
confirmation, or focus the parent tree.

## Keyboard reference

Press `?` in the app for the complete list.

| Key | Action |
| --- | --- |
| `1` / `2` / `3` | Processes / Locked files / Ports |
| `↑` `↓` `PgUp` `PgDn` `Home` `End` | Move (`j`, `g`, `G` also work) |
| `Enter` | Open process details |
| `Esc` | Back, clear search, or clear selection |
| `/` | Edit search; `Enter` applies, `Esc` cancels |
| `Tab` / `→` | Focus the parent tree |
| `i` | Show or hide the side panel |
| `n` / `p` / `m` / `c` | Sort by name / PID / RAM / CPU |
| `s` | Ports: switch between THIS PATH and ALL PORTS |
| `f` / `p` / `l` | Details: files / ports / lock evidence only |
| `Space` / `Ctrl+A` | Select one / all visible |
| `k` / `x` / `K` / `X` | Terminate / force-kill (see [actions](#process-actions)) |
| `r` / `F5` / `a` / `z` | Refresh / refresh / auto-refresh / cancel scan |
| `u` / `U` / `b` | Check for release / open release page / dismiss notice |
| `R` / `D` | Open the GitHub repository / donation page |
| `q` / `Ctrl+C` | Quit (`q` types a letter while editing search) |

<a id="terminal-languages"></a>

## Languages

The interface and `--help` are available in English, German, and Simplified
Chinese:

```sh
oflh --language de .
oflh --language zh --help
```

The default, `--language system`, uses the first non-empty `LC_ALL`,
`LC_MESSAGES`, or `LANG`, then the Windows user locale. Region variants such as
`de_DE.UTF-8` or `zh-CN` resolve to the matching language; anything else falls
back to English. Shortcuts, search tokens (`read`, `mapped`, `locked`, `tcp`,
`port:3000`), file names, and OS error text are the same in every language.

## Release notices

Release builds check for a newer stable version at startup and an hour after
each check. If one exists, a single footer line says so; nothing else on screen
moves. Press `u` to check now, `U` to open the release page, or `b` to hide the
notice for this session. oflh never installs updates itself.

The check fetches a version manifest from the OFLH website over HTTPS, with
GitHub as a fallback. It sends no paths, process data, or credentials. It does
not use proxy settings, so it may fail behind a proxy; automatic failures are
silent. Disable it with `--no-update-check`. Development builds never check.

<a id="terminal-support"></a>

## Terminal requirements

Any terminal emulator with Unicode and true-color support works. Wider windows
show the side panel; narrow ones switch to a compact layout. Confirmations need
enough room to list their targets and will ask you to enlarge the window
otherwise.

### Windows drive roots and paths

Inspect a drive root with `oflh 'C:\'` in PowerShell or `oflh C:\` in Command
Prompt. Paths are displayed in their familiar drive or UNC form (`C:\`,
`\\server\share`) while oflh keeps the exact native path internally. Names that
only make sense in the extended namespace (devices, reserved names, trailing
dots or spaces, alternate streams) keep their `\\?\` prefix. On Linux and macOS,
a backslash is an ordinary filename character.
