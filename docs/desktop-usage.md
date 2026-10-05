# Using OFLH Desktop

Find which processes are using a file, folder or local port without opening a
terminal. For installation and the first scan, see the [README quick start](../README.md#getting-started).
The [terminal guide](terminal-usage.md) covers the separate CLI/TUI interface.

## Linux archive installation

New builds provide `oflh-desktop.linux.amd64.tar.gz` (x86-64) and
`oflh-desktop.linux.arm64.tar.gz` (AArch64) alongside the `.deb` and `.rpm`
installers. Older releases such as `v0.4.0` do not include these archives.
ARM32 builds are not available. Download the archive matching your CPU from
[GitHub Releases](https://github.com/karimz1/open-file-lock-handle/releases),
check it against the release's `checksums.txt`, then extract and run it in your
graphical desktop session. For x86-64:

```sh
tar -xzf oflh-desktop.linux.amd64.tar.gz
./oflh-desktop/oflh-desktop
```

No package conversion, administrator privileges, or `PATH` changes are needed
to launch the extracted app. Keep the directory in a location you can write to.
It includes the executable, license, icon, and startup instructions. Updates are
manual: close the app, download the new archive, and extract it into a fresh directory.

The archive includes the executable, not its system libraries. It requires a
compatible glibc runtime (built on Ubuntu 24.04), GTK 3, WebKitGTK 4.1, libsoup 3,
and their runtime dependencies. Alpine/musl and older incompatible glibc systems
are not supported by these binaries. A tarball does not guarantee compatibility
with every Linux distribution.

On Arch Linux, install the runtime packages before launching:

```sh
sudo pacman -Syu webkit2gtk-4.1 gtk3
```

See Arch's official [WebKitGTK 4.1](https://archlinux.org/packages/extra/x86_64/webkit2gtk-4.1/)
and [GTK 3](https://archlinux.org/packages/extra/x86_64/gtk3/) package pages.
CI checks archive extraction and startup as an unprivileged user on Ubuntu and
Fedora for both architectures, and on Arch Linux for x86-64. Arch Linux ARM
is a separate distribution and is not covered by this Arch smoke test.

## Inspect files and processes

Drop a file or folder anywhere in the window, choose **Open file** / **Open folder**,
or type a path and choose **Inspect**. **Recent targets** lets you revisit targets
from previous launches. Folder scans include descendants.

**Processes** groups results by process. **File usages** shows individual matching
observations. Click a row to open details. Use the panel button or close button to
close it. Table paths are shortened for readability. Full paths and copy/reveal
actions are available in details. Drag column dividers or the details panel edge
to resize. Double-click a column divider to fit that column to its heading and
all matching values, including results outside the visible viewport. A focused
divider also supports Enter to auto-fit and arrow keys to resize.

Details show **Matching handles**, **Local ports**, and **Process ancestry**.
The ancestry tree lists parents above the selected process. Click a parent to
inspect it or use its process actions.

An open file is not proof of a lock. **Lock evidence only** restricts results to
reported evidence. Windows resource users are not proven lock owners. Open
**coverage notices** below the table for permissions and scan limitations.

## Search and filters

Search checks process names, PIDs and full paths using the Rust matcher. A shared
folder name can match every row. To search only names, open **Column filters**
and fill **Process name**. Filters combine with the main search and process scope.

Column filters include exact PID, path, CPU and memory bounds, evidence, and
access/relation. Numeric bounds are inclusive. Unknown metrics do not match a
numeric bound. An unavailable CPU sample is not zero. Choose **Apply filters**
to apply, or **Clear filters** to reset column predicates.


In **Ports**, search `port:3000` for an exact port or `30` for matching fragments.
You can combine terms such as `port:3000 tcp`, `udp`, `ipv6` or `pid:1234`.
The view lists local TCP listeners and bound UDP sockets, not network reachability.
**Target processes only** limits owners to processes seen using the inspected path.
Opening **Local ports** from details scopes results to that captured process.
Refresh and termination preserve that scope: an empty result can confirm that the
process's bindings disappeared. Clear the scope explicitly to see other owners.

The **Auto** control can repeat a completed scan at a chosen interval. Its menu
follows your theme and interface font size. Automatic refresh starts disabled
and lasts only for the current session. The information popup closes when you
click outside it, move focus away, or press Escape.

## Selection and copying

Click a row for details. Ctrl/Cmd-click adds or removes a process. Shift-click
selects a range. Selection represents processes, so several file rows belonging
to one process can highlight together. Select All applies to the filtered results.
Selections can include processes outside the current view. The action bar and
confirmation disclose that.

Use the context menu or details actions to copy paths, filenames, process names,
or PIDs. Ctrl/Cmd+C copies selected rows while the table is focused. Reveal opens
the OS file manager for the selected native path.

## Process actions

To free a file or port, close the application using it normally when possible.
Stopping its process can release the files and ports it holds. Save your work
first: unsaved changes can be lost. **Terminate** requests the
normal platform action. **Force terminate** is a separate, stronger action.
Review the named processes and PIDs before confirming. Cancel is the default.
Stopping a parent process can affect its children or your session.

oflh checks that each PID still belongs to the selected process before acting.
Results refresh afterward. Permission failures and processes still running are
reported. After a failed normal termination request or a verified still-running
process, the results dialog suggests **Force terminate** instead of **Refresh
again**. This opens a new confirmation for only the unsuccessful captured targets,
including a selected ancestor. Cancel remains the default. Force termination
skips normal cleanup and may lose unsaved work; permissions and identity checks
still apply. Unknown exit checks and changed or protected identities do not offer
this recovery action. Normal termination never escalates automatically.

### Administrator retry

When a normal or force termination fails specifically because of permissions,
**Retry with administrator privileges…** opens a fresh confirmation for only the
permission-denied original targets, including ancestors and hidden selections.
The normal or force mode stays the same. Cancel is the default; elevation and
termination never happen automatically.

The UI stays unprivileged. A headless instance of the same executable rechecks
PID plus birth identity and protected-process guards before acting. Windows uses
UAC, Linux uses `/usr/bin/pkexec` (polkit and an authentication agent must be
available), and macOS uses the system administrator authorization prompt through
`osascript`. The OS may request authorization separately for each target.
Cancelling authorization stops the remaining requests. Already elevated sessions
and elevated failures do not offer another administrator retry.

Administrator privileges do not guarantee termination or prove a file lock.
Protected processes and OS restrictions still apply. Exit verification may remain
unavailable to the unprivileged UI even after a successful privileged request.

## Keyboard and refresh

The full shortcut list is shown in Settings and beside relevant controls. Press
`F5` or `Ctrl/Cmd+R` to refresh. Choose an automatic refresh interval beside the
Refresh button. Automatic scans pause during active scans and process-action
confirmations. A progress panel blocks workspace actions during inspection,
shows elapsed time and work counts, and offers Cancel. The results table keeps
its position underneath it. See [long-running inspections](#long-running-inspections).

## Appearance

The gear is the last item at the bottom left and opens a menu with **Settings**
and **Themes**. The GitHub star sits at the bottom right next to **Donate**.
The Themes submenu offers Light, System, Rider Dark, VS Code Dark and OFLH Purple,
with a check beside the design currently in use. **System** follows your device:
dark mode uses VS Code Dark, and light mode uses Light. The current design is
marked **Used by System** while automatic appearance is enabled. Choosing a
concrete design disables automatic appearance.

Focus outlines appear during keyboard navigation, without pre-highlighting
actions when you open a menu or dialog with the mouse. Process confirmations
still default to Cancel.

Settings also offers font size. Your theme, font size and details width persist.
The first launch offers a theme choice. The app remembers these settings between
launches.

In Settings, choose English, German, or Simplified Chinese under **Language**,
or follow your system language. Your preference is saved between launches.

## Updates

OFLH Desktop checks for a newer release at startup and every hour while open.
These background checks stay quiet; an available update adds a **1** badge to
the gear. **About OFLH** sits above the update action at the bottom of the gear
menu. Its dialog shows the installed version, commit (when available), system,
and license, with an option to copy these details. Checks pause while
you review an update dialog or install an update.
A failed background check keeps any known update badge.

Choose **Check for updates** in the gear menu for an immediate check. If you
already have the latest version, a toast confirms it; a failed check also shows
feedback. When a new version is found, a dialog offers release notes and asks
whether to install it. You can also open this dialog from **New update available**
in the gear menu. Choose **Later** to keep working; the badge stays visible.
The **Updates** section in Settings shows the same result and lets you check again.

On Windows and macOS, **Install and restart** downloads the signed update,
installs it, and relaunches the app after you confirm in the dialog. On Linux,
the dialog explains that automatic installation is unavailable for this package.
Choose **Go to download page** to open the
[download page](https://oflh.karimzouine.com/#download) and download the latest
version for your operating system; install the new package or replace your
extracted archive manually. A failed check or installation can be retried.
The installed version appears in Settings under **About OFLH**, where it links
to its GitHub release. Development and RC builds also provide a pipeline link.

For OS limitations, see [platform support](platform-support.md). For build,
testing and packaging details, see [development](development.md).

## Support OFLH

Choose **Donate** to open the support dialog, then choose Buy Me a Coffee,
GitHub Sponsors, or PayPal. The selected service opens in your browser.

## Long-running inspections

While an inspection is running, a modal progress panel blocks workspace actions and reload shortcuts. It shows elapsed time, the current stage and native work counts. The total is unknown; the bar does not imply a percentage. Windows file counts are distinct from descriptors, mappings or module references. Results may still be partial because of permissions or platform limits.

Use **Cancel** to keep the previous accepted results and stop the current request cooperatively. A native call already running can finish later; its results cannot replace the accepted snapshot. F5, automatic refresh and other targets cannot restart an active inspection. Automatic refresh waits a full selected interval after completion or cancellation.
