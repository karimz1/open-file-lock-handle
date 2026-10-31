# Windows 11 context menu payload

CI fills this folder with `oflh_shell_ext.dll`, `oflh-context-menu.msix` and
`oflh-shell-reg.exe` (see `crates/oflh-shell-ext/package/package.ps1`) before the NSIS
bundle step. The installer copies it to `<install dir>\shell-ext\` and only
registers the identity package when the context menu option is checked.

A local build without these files still works: the installer then writes only
the classic context menu entries. Do not commit the generated files.
