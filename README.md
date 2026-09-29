<p align="center">
  <img src="images/oflh-logo.svg" alt="Open File Lock Handle (oflh)" width="760">
</p>

<a id="oflh--find-locked-files-and-the-processes-using-them"></a>

# oflh — Find which process is using a file

[![Platforms](https://img.shields.io/badge/platforms-Linux_%C2%B7_macOS_%C2%B7_Windows-64748b?style=flat)](#platform-behavior)
[![GitHub Downloads](https://img.shields.io/github/downloads/karimz1/open-file-lock-handle/total.svg)](https://github.com/karimz1/open-file-lock-handle/releases)
[![CI](https://github.com/karimz1/open-file-lock-handle/actions/workflows/ci.yml/badge.svg)](https://github.com/karimz1/open-file-lock-handle/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

[![Listed on AwesomeTUI](https://img.shields.io/badge/AwesomeTUI-listed-64748b?style=flat)](https://awesometui.com/open-file-lock-handle)
[![Listed on AlternativeTo](https://img.shields.io/badge/AlternativeTo-listed-64748b?style=flat)](https://alternativeto.net/software/oflh-open-file-lock-handle/about/)

**Open File Lock Handle (`oflh`) finds processes using a file or directory on
Windows, Linux, and macOS.** Choose the graphical **OFLH Desktop** or the
**CLI with interactive terminal UI (TUI)** to investigate locked files, open
handles, and mapped files. Both use the same Rust inspection engine. It also finds **TCP listening ports and bound UDP sockets**, shows their
processes, and links them to file usage in your project. Start with a path to see which processes reference it and what they have open.

[Install](#installation) · [Quick start](#getting-started) ·
[Desktop guide](docs/desktop-usage.md) · [Terminal guide](docs/usage.md) ·
[Platform support](#platform-behavior)

## Two interfaces, one inspection engine

| | OFLH Desktop · release candidate | CLI / TUI |
|---|---|---|
| Start with | Drag a file or folder into the window | `oflh ./build` or `oflh --port 3000` |
| Investigate | Resizable tables, column filters, process details and ancestry | Keyboard navigation, fuzzy search and ancestry |
| Customize | Light, Rider Dark, VS Code Dark and OFLH Purple; adjustable font size | Compact terminal interface |
| Get started | [Desktop downloads](#desktop-downloads) · [Desktop guide](docs/desktop-usage.md) | [Install the CLI](#installation) · [Quick start](#getting-started) |

### Desktop

![OFLH Desktop showing a searchable process table and a process details panel with ancestry](images/desktop.png)

Desktop preview rendered from synthetic test data. Desktop is an RC: see the
[validation checklist](docs/desktop-rc.md) for coverage and remaining native checks.

### Terminal

<a href="images/demo.gif">
  <img src="images/demo.gif" alt="oflh terminal UI showing processes using a target path, file access modes, and process ancestry" width="100%">
</a>

Demo with sample processes and paths.
[Watch the terminal recording](https://asciinema.org/a/HVwfkoVJfOi5ckDa).

## When to use oflh

- **A file is “in use by another process.”** Find processes referencing it before retrying a rename, move, or delete.
- **A build cannot replace a DLL or executable.** Inspect the output directory to find a running application or development tool still using its files.
- **You want to clean up a directory.** See which visible processes reference files beneath it, then inspect their file usages and parent processes.
- **You are troubleshooting a file lock.** Switch to the Locked files view to examine the lock or sharing-conflict evidence available on your OS.

An open file is **not necessarily locked**. `oflh` separates file usage from lock
evidence and does not directly unlock files. Stopping a process may release its
resources; it can also interrupt work in that application.

<a id="what-makes-it-different"></a>

## Why oflh?

Use it alongside `lsof`, `fuser`, or Task Manager when you want to start with a
path and investigate interactively, without knowing a process name or PID.

- **Search quickly:** filter process names and file paths with fragments, CamelCase abbreviations, and wildcards.
- **Inspect the context:** view individual file usages, access modes, parent processes, CPU, and memory.
- **Follow changes:** sort results, rescan on demand, or enable five-second auto-refresh in the TUI.
- **Act from the same interface:** request termination with confirmation and process identity checks.

Written in Rust, `oflh` uses native OS interfaces. See [platform behavior](#platform-behavior)
for discovery coverage and lock-detection limits.

## Installation

### Desktop downloads

Get **OFLH Desktop** from [GitHub Releases](https://github.com/karimz1/open-file-lock-handle/releases).
Choose a release that includes Desktop assets, then choose your OS and CPU:

| Platform | Desktop package naming for the upcoming RC |
| --- | --- |
| Linux | `oflh-desktop.linux.amd64.deb` / `.rpm`, or `arm64` |
| macOS | `oflh-desktop.darwin.amd64.dmg`, or `arm64` for Apple Silicon |
| Windows | `oflh-desktop.windows.amd64.exe`, or `arm64` |

These are direct installers, not ZIP bundles. **The currently published v0.1.1
contains CLI binaries only; Desktop packages are being prepared for the next RC.**
**Windows:** download the Desktop `.exe` installer from that release page.
**macOS:** the tap is being prepared for `brew install --cask karimz1/tap/oflh-desktop`;
the cask becomes available after the first stable Desktop release and tap update.
For Desktop RCs, use the release-page DMG. The formula below installs the terminal app. Unsigned installers may
show an OS publisher warning; publisher metadata is not a signing certificate.

### CLI / TUI — Homebrew

<a id="homebrew"></a>

On macOS or Linux:

```sh
brew install karimz1/tap/oflh
```

Update with `brew update` followed by `brew upgrade oflh`.

### CLI / TUI — standalone executable

<a id="standalone-binaries"></a>

Download a terminal executable from [GitHub Releases](https://github.com/karimz1/open-file-lock-handle/releases).
The **published v0.1.1** names are:

| Platform | x86-64 (Intel / AMD) | ARM64 |
| --- | --- | --- |
| Linux | `oflh-linux-amd64` | `oflh-linux-arm64` |
| macOS | `oflh-darwin-amd64` | `oflh-darwin-arm64` |
| Windows | `oflh-windows-amd64.exe` | `oflh-windows-arm64.exe` |

Starting with the combined Desktop/CLI RC, terminal downloads use explicit names
such as `oflh-cli.linux.amd64`, `oflh-cli.darwin.arm64`, and
`oflh-cli.windows.amd64.exe`. The installed command stays **`oflh`**.

In the download folder, rename your executable to `oflh` (Windows: `oflh.exe`).
On Linux or macOS:

```sh
chmod +x ./oflh
./oflh "/path/to/project"
```

On Windows, in PowerShell:

```powershell
.\oflh.exe "C:\projects\example"
```

No `PATH` change is needed. To use `oflh` from any folder, move the executable to
a directory on your `PATH`. Releases include one `checksums.txt` for SHA-256
verification of all CLI and Desktop downloads included in that release.

<a id="build-from-source"></a>
<a id="oflh-desktop-from-source"></a>

Building either app yourself? See [development and source builds](docs/development.md).

## Getting started

### Desktop quick start

1. Launch **OFLH Desktop** and choose your theme.
2. Drop a file or folder into the window, or choose **Open file** / **Open folder**.
3. Click a result to see full paths, matching handles, local ports and process ancestry.
4. Use search or **Column filters** to narrow results. Press **F5** to refresh.
5. Close the owning application normally when possible. If necessary, use **Terminate**
   and review the confirmation; force termination can lose unsaved work.

See the [Desktop guide](docs/desktop-usage.md) for selection, ports, filters,
keyboard shortcuts and appearance settings.

### Terminal quick start

Run `oflh [PATH]` in an interactive terminal. Without a path, it inspects the
current directory. Folder targets include their descendants.

```sh
oflh ./build             # Files used inside a folder
oflh ./build/plugin.dll  # A file or DLL in use
oflh --port 3000         # A local TCP/UDP port
oflh --help
```

Use quotes for paths containing spaces. In PowerShell, for example:
`oflh "C:\projects\example\build\plugin.dll"`.

Select a process in **Processes** (`1`), press **Enter** for its file usages,
and press **r** to refresh. The [terminal guide](docs/usage.md) explains the full workflow.

<a id="search"></a>
<a id="find-processes-using-ports"></a>

### Search and ports

Both apps search file usage and local TCP listeners / bound UDP sockets.
`port:3000` matches an exact port; bare `30` matches port-number fragments.
Open **Ports** in Desktop or press **3** in the TUI. Unknown owners cannot be
terminated, and a bound port does not prove network reachability.

See [Desktop search and filters](docs/desktop-usage.md#search-and-filters) or
[terminal port search](docs/usage.md#ports) for scope and matching rules.

## Keyboard reference

| Action | Desktop | Terminal |
| --- | --- | --- |
| Processes / file usages / ports | `Ctrl/Cmd+1` / `2` / `3` | `1` / `2` / `3` (2 = Locked files) |
| Search | `Ctrl/Cmd+F` or `/` | `/` |
| Refresh | `F5` or `Ctrl/Cmd+R` | `r` |
| Open details | Click a row or `Enter` | `Enter` |
| Settings / help | Sidebar **Settings** | `?` |

See all [Desktop shortcuts](docs/desktop-usage.md#keyboard-shortcuts) or
[terminal shortcuts](docs/usage.md#keyboard-reference).

<a id="process-actions"></a>

Both interfaces confirm destructive actions and validate process identity.
Selections can include hidden results; review the confirmation before terminating.
See [Desktop actions](docs/desktop-usage.md#process-actions) or
[terminal actions](docs/usage.md#process-actions), especially before stopping a parent process.

<a id="file-discovery-and-termination"></a>
<a id="lock-evidence"></a>
<a id="access-modes"></a>
<a id="cpu-memory-and-ancestry"></a>

## Platform behavior

CLI binaries are available for **Linux, macOS, and Windows on x86-64 and ARM64**.
Desktop packaging targets the same six combinations; check the chosen release for available installers.
The interface is shared, but file discovery and lock detection depend on the OS.

| Platform | File usage discovery | Lock evidence |
| --- | --- | --- |
| Linux | Open descriptors, working directories, executables, mapped files, deleted-but-open files via `/proc` | Held FLOCK, POSIX, and OFD locks |
| macOS | Vnode descriptors, working directories, executables, mapped files via `libproc` | POSIX byte-range conflicts; flock-only locks may be missed |
| Windows | Restart Manager resource users, modules and executables via Toolhelp | Read, write, or delete sharing conflicts; reported owners are unverified |

On Windows, a sharing conflict does not prove which reported process caused it.
Discovery does not cover working directories, directory handles, or deleted files,
and byte-range locks are not enumerated.

Results are a snapshot limited by permissions, process exits, and concurrent file
activity. In Desktop, open **coverage notices** below the table; in the TUI,
press `?` for scan limitations. See [platform support and limitations](docs/platform-support.md) for the
full detection scope, termination behavior, and metric definitions.

## Common questions

### Can oflh unlock a file?

It can help you find and stop a process using the file. It does not remove locks
directly or bypass OS permissions. Close the application normally first when
possible; force killing can lose unsaved work.

### Why is a process listed but the Locked files view is empty?

File usage and file locks are different. A process can have an open descriptor
or mapped file without holding a detectable lock. The Locked files view requires
additional evidence, and each platform has detection limits.

### Why are some processes or locks missing?

Permissions and platform coverage limit what can be inspected. Elevated
privileges may improve visibility, but cannot guarantee complete results. On
Linux, inspecting another container may require running `oflh` inside its mount
namespace. Check `?` for scan warnings and the [platform reference](docs/platform-support.md).

### Does oflh support scripts or JSON output?

The CLI requires an interactive terminal; `oflh` currently has no JSON
or non-interactive scan output. `--help` and `--version` work without a TTY.

<a id="performance"></a>
<a id="testing-and-development"></a>
<a id="terminal-recommendation"></a>

## Documentation and development

- [Desktop guide](docs/desktop-usage.md): graphical workflows, filters and shortcuts.
- [Terminal guide](docs/usage.md): search syntax, keyboard shortcuts, process actions, and terminal support.
- [Platform reference](docs/platform-support.md): discovery, lock evidence, and permissions.
- [Contributing](CONTRIBUTING.md) and [development](docs/development.md): build and test instructions.
- [Architecture](docs/architecture.md): native backends and safety boundaries.
- [Performance measurements](docs/performance.md): benchmark methodology, results, and the historical Go-to-Rust comparison.
- [Releasing](docs/releasing.md): maintainer packaging instructions.

Run `cargo xtask check` for formatting, Clippy, and workspace tests. CI covers
native Linux, macOS, and Windows on x86-64 and ARM64.

<a id="project-information"></a>

`oflh` is in beta. [Report a bug](https://github.com/karimz1/open-file-lock-handle/issues)
with the version, operating system, and steps to reproduce it.

Licensed under [MIT](LICENSE). [Support development](https://buymeacoffee.com/karimz1).
