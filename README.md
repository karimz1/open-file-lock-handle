<p align="center">
  <img src="crates/oflh-desktop/app-icon.svg" alt="Open File Lock Handle Desktop app icon" width="88">
</p>

# Open File Lock Handle (oflh)

[![GitHub downloads](https://img.shields.io/github/downloads/karimz1/open-file-lock-handle/total?label=downloads)](https://github.com/karimz1/open-file-lock-handle/releases)
[![CI](https://github.com/karimz1/open-file-lock-handle/actions/workflows/ci.yml/badge.svg)](https://github.com/karimz1/open-file-lock-handle/actions/workflows/ci.yml)
[![Platforms](https://img.shields.io/badge/platforms-Linux%20%7C%20macOS%20%7C%20Windows-64748b)](#platforms)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Listed on AwesomeTUI](https://img.shields.io/badge/AwesomeTUI-listed-64748b)](https://awesometui.com/open-file-lock-handle)
[![Listed on AlternativeTo](https://img.shields.io/badge/AlternativeTo-listed-64748b)](https://alternativeto.net/software/oflh-open-file-lock-handle/about/)

Find which processes are using a file or directory, and which processes own local
TCP listeners or bound UDP sockets. Choose OFLH Desktop or the interactive
terminal app (CLI/TUI). Both use the same native Rust inspection engine on Linux,
macOS, and Windows.

## Choose an interface

| | OFLH Desktop | oflh CLI/TUI |
| --- | --- | --- |
| Best for | Exploring results in a graphical workspace | Working from a terminal with keyboard navigation |
| Includes | File and folder scans, ports, filters, process details and ancestry | File and folder scans, ports, process details and ancestry |
| Start with | Open the app, then choose or drop a file or folder | `oflh [PATH]` or `oflh --port PORT` |
| Install | [Desktop options](#desktop-install) | [CLI/TUI options](#cli-tui-install) |

### Desktop

![OFLH Desktop showing process results and process details](images/desktop.png)

### Terminal

[![asciicast](https://asciinema.org/a/1266562.svg)](https://asciinema.org/a/1266562)

## Install

<a id="cli-tui-install"></a>

### CLI/TUI

On macOS or Linux, install with Homebrew:

```sh
brew install karimz1/tap/oflh-cli
```

The existing `brew install karimz1/tap/oflh` name remains available as a
compatibility alias. Homebrew installs the terminal app. It does not install
OFLH Desktop.

For a published prerelease, use `brew install karimz1/tap/oflh-cli-rc`. The
stable and RC formulae both provide `oflh`, so install only one at a time.

For a standalone CLI, download the executable for your operating system and CPU
from [GitHub Releases](https://github.com/karimz1/open-file-lock-handle/releases).
Release names include the platform and architecture. Rename the file to `oflh`
if you want a shorter command, then run it in a terminal with a file, directory,
or port as its target.

For options and keyboard controls, see the [Terminal user guide](docs/usage.md).

<a id="desktop-install"></a>

### Desktop

Download an installer from [GitHub Releases](https://github.com/karimz1/open-file-lock-handle/releases):

| Operating system | Install method |
| --- | --- |
| macOS | Homebrew Cask for stable or prerelease builds, or the DMG from GitHub Releases |
| Linux | DEB or RPM package for x86-64 or ARM64 |
| Windows | Download the installer from the Releases page |

Install stable Desktop with `brew install --cask karimz1/tap/oflh-desktop` or a
published RC with `brew install --cask karimz1/tap/oflh-desktop-rc`. The stable
and RC Casks install the same app and cannot be installed side by side. Linux
Desktop users install native DEB/RPM packages. Homebrew Cask is for macOS.

For search, filters, and process actions, see the
[Desktop user guide](docs/desktop-usage.md).

Release downloads are individual files, not ZIP bundles. Each release includes
one `checksums.txt` with SHA-256 hashes for every CLI executable and Desktop
installer in that release.

<a id="getting-started"></a>

## Quick start

### Desktop

1. Open a file or folder, or drop it into the window.
2. Select a process to inspect its matching file usages, ports, and ancestry.
3. Search or filter the results, then refresh to take a new snapshot.
4. Close the owning application normally when possible. Termination is an
   explicit, confirmed action and can interrupt work.

### CLI/TUI

Run `oflh` in an interactive terminal. With no argument, it scans the current
directory. A directory target includes descendants.

```sh
oflh ./build
oflh ./build/plugin.dll
oflh --port 3000
```

Use `1`, `2`, and `3` to switch between Processes, Locked files, and Ports. Press
Enter to inspect a selected process, `/` to search, `r` to refresh, and `?` for
help.

## What the results mean

An open file is not necessarily locked. OFLH separates file usage from lock
evidence and does not unlock files. Detection depends on operating-system APIs,
permissions, and concurrent system activity. Windows resource users are not
verified lock owners. See [platform coverage and limitations](docs/platform-support.md).

## Platforms

CLI and Desktop releases target Linux, macOS, and Windows on x86-64 and ARM64.
Desktop installers are DEB/RPM on Linux, DMG on macOS, and EXE on Windows. The
release page is the source of truth for available builds.

## Project references

- [Platform support and limitations](docs/platform-support.md)

For contributors and maintainers:

- [Architecture](docs/architecture.md)
- [Development and testing](docs/development.md)
- [Performance measurements](docs/performance.md)
- [Release process](docs/releasing.md)

Report a reproducible problem in [GitHub Issues](https://github.com/karimz1/open-file-lock-handle/issues).
OFLH is licensed under [MIT](LICENSE).
