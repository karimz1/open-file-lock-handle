# Terminal user guide

[Back to the README](../README.md) · [Platform behavior](platform-support.md)

## File inspection

For installation and a first scan, see the [README quick start](../README.md#getting-started).
A file target shows references to that file; a folder target includes descendants.
**Processes** groups usages by process. **Locked files** shows only observations
with lock or sharing-conflict evidence. Press `Enter` to inspect a process.

Press `r` or `F5` to refresh, or `a` for automatic refresh five seconds after the previous inspection finishes. Active inspections ignore further reloads. Press `z` to cancel; native calls already running may finish before cancellation is acknowledged. Progress shows elapsed time and attempted process/resource counts, not a completeness total. Accepted rows remain usable during refresh; the last successful scan duration stays visible. Automatic refresh pauses while editing search, reviewing confirmations/help or focusing the ancestry tree. In details, `l` shows
only lock evidence. Search and filters stay active when you refresh. Use `←` and
`→` to read a long path. On wider terminals, the side panel also shows process
parents, CPU, and memory.

## Ports

Press `3` for **Ports**, or start directly with `oflh --ports`. The default is all
visible local bindings when no path is supplied. An explicit path starts in
**THIS PATH**: `oflh --ports ./project` shows ports of processes associated with
that project, and `oflh --port 5040 ./project` adds an exact port filter.
Press `s` to switch between this path and all ports. File and port search text are
independent, so switching tabs does not mix their filters.

| Query | Meaning |
| --- | --- |
| `50` | Port numbers containing 50, such as 5040; updates while typing |
| `port:3000` | Exact local port; does not match `13000` or a PID |
| `tcp port:3000` | TCP listeners on port 3000 |
| `udp` | Bound UDP sockets |
| `ipv6` | IPv6 bindings |
| `127.0.0.1` | Bindings matching that address |
| `pid:424242` | Search the process ID explicitly |
| `node port:3000` | Both the process text and exact port must match |

Exact ports must be between 1 and 65535. Each address, protocol, and port has its
own row; IPv4 and IPv6 are separate. Other text follows the search rules below.

**THIS PATH** shows ports of processes observed using the target in the header.
This does not prove the project created those sockets. On Windows, some development
servers appear only in **ALL PORTS** because working directories cannot be inspected.

Press `Enter` for the owner's port details, `f` for its file usages, and `p` to
return to ports. `Esc` goes back; if you switched between detail views, it returns
to the original detail view first. File and port searches remain separate.

If you started without a path, inspecting an owner or switching to a file tab
scans its working directory, or its executable's folder if that is unavailable.
Check the header for the chosen folder. Failed scans keep the previous target.
Supplying a path on the command line keeps that target fixed.

Port counts describe the whole process, regardless of the file search. Zero
means none detected. **Owner unavailable** rows cannot be terminated. Selecting
several ports of one process selects that process only once.

## Search

Search is case-insensitive and updates as you type. The same rules apply to
Processes, Locked files, and file-usage details. Ports use partial numeric matching and explicit exact port terms as described above.

| Query | Meaning |
| --- | --- |
| `dll` | Text within a name or path, such as `plugin.dll` |
| `MIMJWT` | Initials of words in `Microsoft.IdentityModel.JsonWebTokens.dll` |
| `micro*dll` | `micro` followed by `dll`, with any text between them |
| `FLEC.` / `FLEC*` | Shortened name matching `FileLockExampleCli.dll` or `FileLockExampleCli.deps.json` |
| `FLEC*.json` | Shortened name followed by `.json` |
| `*.dll` | A field containing `.dll` |
| `micro*dll mapped` | Both terms must match |

Search matches parts of names and paths, or the beginnings of words in a name.
It does not match arbitrary scattered letters. `*` allows any text, including
folder separators; each part around it also accepts shortened names. Punctuation
is literal: `FLEC.` needs a dot after the shortened name. Patterns match anywhere,
so `*.dll` can also match `plugin.dll.backup`. All space-separated terms must match.

Filename and process-name matches rank above folder-only matches unless you choose
a sort such as CPU or PID. The matching path and `+N` count reflect your filter.
File-related search terms carry into details; process-only terms stay in the main
view. Clear the detail search with `/`, `Ctrl+U`, then `Enter` to see every usage.

## Process actions

Press `Space` to select processes, or `Ctrl+A` to select or deselect all visible
processes. Selections survive filtering, so a selection may include processes
that are no longer visible.

- `k` requests normal termination of the selection, or the current process if nothing is selected.
- `x` force kills the same targets.
- Every action requires confirmation, with **Cancel** selected by default. The dialog lists the targets, including hidden selections.

To act on a parent, press `Tab` or `→` to focus the ancestry tree. It initially
selects the current process. Use `↑` to move toward its parents and `↓` to return
toward the current process. Here, `k` and `x` apply only to the highlighted tree
node, regardless of selections in the main list.

The tree keeps the processes it captured across refreshes. oflh checks that a PID
still belongs to the same process before acting. Protected processes and parents
that could not be identified cannot be stopped. Results refresh after a successful
request. A termination request does not guarantee the process has exited.

Stopping a parent may close its application and affect its children. It does not
recursively terminate the entire tree. To free a file or port, close the application
normally or use these actions to stop the process holding it. Once the process
exits, its resources can be released. Save your work before stopping a process.

## Keyboard reference

Press `?` for the full shortcut list. The main controls are:

| Key | Action |
| --- | --- |
| `1` / `2` / `3` | Processes / Locked files / Ports |
| `↑` / `↓`, `Enter` | Select and inspect |
| `/`, then `Enter` / `Esc` | Edit search, then apply / cancel |
| `Space` / `Ctrl+A` | Select one / all visible processes |
| `r` / `F5` / `a` | Refresh / refresh / auto-refresh after completion |
| `z` | Cancel active inspection |
| `k` / `x` | Terminate / force kill, with confirmation |
| `Esc` | Go back or clear search |
| `q` / `Ctrl+C` | Quit (`q` enters text while searching) |

`K` / `X` act on the selection, or all filtered processes if nothing is selected.
Review the confirmation carefully. `Tab` changes focus, not views.

## Terminal support

Use an interactive terminal with Unicode and true-color support. A wider window
provides room for the process table and side panel. No particular terminal
emulator is required.
