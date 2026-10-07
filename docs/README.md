# OFLH documentation

Start with the [project README](../README.md) for downloads, installation, and a
first scan. OFLH provides desktop and terminal interfaces for Linux, macOS, and
Windows.

## Using OFLH

| Task | Guide |
| --- | --- |
| Inspect files and folders in the desktop app | [Desktop guide](desktop-usage.md) |
| Identify the parent app behind a background process | [Parent process tree](desktop-usage.md#follow-the-parent-process-tree) |
| Find file users or a process using a local port from the terminal | [Terminal guide](terminal-usage.md) |
| Understand lock evidence, permissions, and OS coverage | [Platform support](platform-support.md) |
| Check measured scan times for Windows `C:\` or Linux `/` | [Inspection performance](inspection-performance.md) |
| Report a bug with a reproducible example | [Contributing](../CONTRIBUTING.md#report-a-bug) |

## Building and maintaining OFLH

| Task | Reference |
| --- | --- |
| Build locally and run checks | [Development](development.md) |
| Understand crate boundaries and native code | [Architecture](architecture.md) |
| Validate inspection changes | [Regression coverage](inspection-regressions.md) |
| Reproduce scan and UI benchmarks | [Inspection performance](inspection-performance.md) |
| Prepare a release and publish updater metadata | [Releasing](releasing.md) |
