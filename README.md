<p align="center">
  <img src="crates/oflh-desktop/app-icon.svg" alt="Open File Lock Handle logo" width="88">
</p>

# Open File Lock Handle (oflh)

[![Website](https://img.shields.io/badge/website-oflh-8B5CF6)](https://oflh.karimzouine.com/)
[![Platforms](https://img.shields.io/badge/platforms-Linux%20%7C%20macOS%20%7C%20Windows-64748b)](#platforms)
[![Desktop languages: English, German, Simplified Chinese](https://img.shields.io/badge/desktop%20languages-English%20%7C%20German%20%7C%20Simplified%20Chinese-4264db)](https://github.com/karimz1/open-file-lock-handle/tree/main/crates/oflh-desktop/ui/src/locales)
[![GitHub downloads](https://img.shields.io/github/downloads/karimz1/open-file-lock-handle/total?label=downloads)](https://github.com/karimz1/open-file-lock-handle/releases)
[![CI](https://github.com/karimz1/open-file-lock-handle/actions/workflows/ci.yml/badge.svg)](https://github.com/karimz1/open-file-lock-handle/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

oflh shows which processes are using a file, folder, or local port. Use it to
investigate files that cannot be deleted or replaced, folders still in use, and
“port already in use” errors.

Choose the desktop app or the interactive terminal app. Both run on Linux,
macOS, and Windows. You can inspect matching file usages, lock evidence, process
parents, and local ports. Filter for lock evidence and, after confirmation,
terminate a process to help release the files or ports it holds.

[Install](#install) · [Build from source](#build-from-source)

## Desktop

![OFLH Desktop showing processes using a folder and the selected process details](images/desktop.png)

## Terminal

[![Terminal demonstration](images/demo-thumbnail.jpg)](images/demo.gif)

[Watch the terminal recording](https://asciinema.org/a/1266562)

## Install

Download the build for your operating system and CPU from
[GitHub Releases](https://github.com/karimz1/open-file-lock-handle/releases).
`amd64` means x86-64; `arm64` includes Apple Silicon. For an easier way to find
the right download and view screenshots, visit the [website](https://oflh.karimzouine.com/).

<a id="desktop-install"></a>

### Desktop

| Platform | Download |
| --- | --- |
| macOS | `.dmg` |
| Linux | `.deb`, `.rpm`, or a manual `.tar.gz` archive (x86-64 / ARM64) |
| Windows | `-installer.exe` |

Open the installer and follow its steps. Desktop installers are unsigned; see
[installation warnings](#unsigned-installers) if your OS blocks them.

Linux archive downloads are included in new builds; older releases such as
`v0.4.0` only contain `.deb` and `.rpm` desktop packages. The archive can be used
on Arch and other compatible glibc distributions after installing system
libraries. See [Linux archive installation](docs/desktop-usage.md#linux-archive-installation)
for extraction, startup, and dependency requirements. ARM32 is not available.

<a id="cli-tui-install"></a>

### Terminal

With Homebrew on macOS or Linux:

```sh
brew install karimz1/tap/oflh-cli
```

Alternatively, download a standalone CLI executable. On macOS or Linux, make it
executable and run it directly. For example, on macOS ARM64:

```sh
chmod +x ./oflh-cli.darwin.arm64
./oflh-cli.darwin.arm64 ./build
```

Use the filename of your download. On Windows x86-64, run from PowerShell:

```powershell
.\oflh-cli.windows.amd64.exe .\build
```

You can rename the executable to `oflh` (`oflh.exe` on Windows) and place it on
`PATH` to use the commands below from any directory.

<a id="getting-started"></a>

## Quick start

### Desktop

1. Open the app and choose **Open file** or **Open folder**, or drop a path into the window.
2. Select a process to inspect its file usages, local ports, and ancestry.
3. Search or filter the results. Use **Lock evidence only** to focus on reported locks.
4. To free a file, close the application normally or terminate its process from
   oflh after confirmation. Save your work first; termination can lose unsaved changes.
5. Refresh to check the current file usage.

See the [Desktop user guide](docs/desktop-usage.md) for filters, selection, and actions.

### Terminal

Run in an interactive terminal. With no path, oflh scans the current directory;
folder scans include descendants.

```sh
oflh ./build                 # Processes using this folder
oflh ./build/plugin.dll      # Processes using one file
oflh --port 3000             # Local TCP listener or UDP binding on port 3000
```

| Key | Action |
| --- | --- |
| `1` / `2` / `3` | Processes / Locked files / Ports |
| `↑` / `↓`, `Enter` | Select and inspect a process |
| `/` | Search |
| `r` | Refresh |
| `?` | Help and scan warnings |
| `q` | Quit (outside search editing) |

See the [Terminal user guide](docs/terminal-usage.md) for search, selection, and actions.

<a id="platform-behavior"></a>

## Limitations

An open file is not always locked. The lock filter shows reported lock evidence;
on Windows, file users and sharing conflicts cannot reliably identify the process
causing a lock. Permissions can limit results. Refresh after closing a program;
an empty result alone does not guarantee a file is free. Ports show local TCP
listeners and bound UDP sockets.

Close the owning application normally when possible. Termination can lose unsaved
work, and stopping a parent can affect its children. Actions require confirmation.
See [platform support](docs/platform-support.md) for detection limits.

## Privacy

Privacy is a core principle of oflh. File and process inspection runs locally;
file contents and scan results are not uploaded. That is why oflh is open source:
anyone can [inspect the code](https://github.com/karimz1/open-file-lock-handle)
and [build the tool themselves](docs/development.md). The build workflows and
[CI runs](https://github.com/karimz1/open-file-lock-handle/actions), including
those for release candidates (RCs), are public too.

The desktop app contacts GitHub to check for and download updates.

## Build from source

See [Development](docs/development.md) for prerequisites and the commands to build
and run either the CLI or Desktop locally.

## Unsigned installers

Downloads are unsigned. Your system may show a publisher or signature
warning, such as “unknown publisher” on Windows. Only bypass a warning for a
download you trust. Releases include `checksums.txt` for checking SHA-256 hashes.

- **macOS:** If Gatekeeper blocks the installed app or reports it as damaged,
  clear its quarantine flag with `xattr -cr "/Applications/OFLH Desktop.app"`,
  then reopen it. Adjust the path if installed elsewhere.

<a id="platforms"></a>

Release targets are Linux, macOS, and Windows on x86-64 and ARM64. Available
packages are listed on the release page.

<a id="project-references"></a>

## Documentation

- [Terminal guide](docs/terminal-usage.md) and [Desktop guide](docs/desktop-usage.md)
- [Platform support and limitations](docs/platform-support.md)
- [Building and testing](docs/development.md)
- [Contributing](CONTRIBUTING.md), [Architecture](docs/architecture.md),
  and [Releasing](docs/releasing.md)

Report bugs in [GitHub Issues](https://github.com/karimz1/open-file-lock-handle/issues).
Licensed under [MIT](LICENSE).
