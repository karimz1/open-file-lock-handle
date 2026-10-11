## Download the desktop app

[Visit the OFLH website for download guidance and documentation](https://oflh.karimzouine.com#download).
The website offers the latest stable release. For release candidates, use the links below for this release.

Choose the desktop app for a graphical window with buttons and a process list. Install it, then open OFLH Desktop. [Desktop guide](https://github.com/karimz1/open-file-lock-handle/blob/main/docs/desktop-usage.md).

| Platform | Download |
| --- | --- |
| Windows · x86-64 | [oflh-desktop.windows.amd64-installer.exe](https://github.com/karimz1/open-file-lock-handle/releases/download/{{TAG}}/oflh-desktop.windows.amd64-installer.exe) |
| Windows · ARM64 | [oflh-desktop.windows.arm64-installer.exe](https://github.com/karimz1/open-file-lock-handle/releases/download/{{TAG}}/oflh-desktop.windows.arm64-installer.exe) |
| macOS · Apple silicon | [oflh-desktop.darwin.arm64.dmg](https://github.com/karimz1/open-file-lock-handle/releases/download/{{TAG}}/oflh-desktop.darwin.arm64.dmg) |
| macOS · Intel | [oflh-desktop.darwin.amd64.dmg](https://github.com/karimz1/open-file-lock-handle/releases/download/{{TAG}}/oflh-desktop.darwin.amd64.dmg) |
| Linux · x86-64 · DEB | [oflh-desktop.linux.amd64.deb](https://github.com/karimz1/open-file-lock-handle/releases/download/{{TAG}}/oflh-desktop.linux.amd64.deb) |
| Linux · ARM64 · DEB | [oflh-desktop.linux.arm64.deb](https://github.com/karimz1/open-file-lock-handle/releases/download/{{TAG}}/oflh-desktop.linux.arm64.deb) |
| Linux · x86-64 · RPM | [oflh-desktop.linux.amd64.rpm](https://github.com/karimz1/open-file-lock-handle/releases/download/{{TAG}}/oflh-desktop.linux.amd64.rpm) |
| Linux · ARM64 · RPM | [oflh-desktop.linux.arm64.rpm](https://github.com/karimz1/open-file-lock-handle/releases/download/{{TAG}}/oflh-desktop.linux.arm64.rpm) |
| Linux · x86-64 · archive | [oflh-desktop.linux.amd64.tar.gz](https://github.com/karimz1/open-file-lock-handle/releases/download/{{TAG}}/oflh-desktop.linux.amd64.tar.gz) |
| Linux · ARM64 · archive | [oflh-desktop.linux.arm64.tar.gz](https://github.com/karimz1/open-file-lock-handle/releases/download/{{TAG}}/oflh-desktop.linux.arm64.tar.gz) |

### macOS installation warning

Because the application is unsigned, macOS may report it as “damaged.” Allow it to run manually with this command in Terminal, then reopen it. Adjust the path if installed elsewhere.

```sh
xattr -cr "/Applications/OFLH Desktop.app"
```

## Download the terminal version (CLI / TUI)

These downloads run inside a terminal. On Windows, open PowerShell and run the executable with a file, folder, or port argument. Double-clicking the CLI executable does not open the desktop app. [Terminal guide](https://github.com/karimz1/open-file-lock-handle/blob/main/docs/terminal-usage.md).

| Platform | Download |
| --- | --- |
| Windows · x86-64 | [oflh-cli.windows.amd64.exe](https://github.com/karimz1/open-file-lock-handle/releases/download/{{TAG}}/oflh-cli.windows.amd64.exe) |
| Windows · ARM64 | [oflh-cli.windows.arm64.exe](https://github.com/karimz1/open-file-lock-handle/releases/download/{{TAG}}/oflh-cli.windows.arm64.exe) |
| macOS · Apple silicon | [oflh-cli.darwin.arm64](https://github.com/karimz1/open-file-lock-handle/releases/download/{{TAG}}/oflh-cli.darwin.arm64) |
| macOS · Intel | [oflh-cli.darwin.amd64](https://github.com/karimz1/open-file-lock-handle/releases/download/{{TAG}}/oflh-cli.darwin.amd64) |
| Linux · x86-64 | [oflh-cli.linux.amd64](https://github.com/karimz1/open-file-lock-handle/releases/download/{{TAG}}/oflh-cli.linux.amd64) |
| Linux · ARM64 | [oflh-cli.linux.arm64](https://github.com/karimz1/open-file-lock-handle/releases/download/{{TAG}}/oflh-cli.linux.arm64) |

Verify downloads against [checksums.txt](https://github.com/karimz1/open-file-lock-handle/releases/download/{{TAG}}/checksums.txt). The Assets list below also includes updater files and signatures.
