# Desktop RC validation

Desktop and CLI/TUI share the Rust inspection engine, but have separate UI tests
and release artifacts. Do not treat a browser test as proof that a native package
works on every OS.

## Automated gates

Run from the repository root:

```sh
cargo xtask check
npm --prefix crates/oflh-desktop/ui ci
npm --prefix crates/oflh-desktop/ui run format:check
npm --prefix crates/oflh-desktop/ui run build
npm --prefix crates/oflh-desktop/ui test
# Install the browser once on a development/test machine:
cd crates/oflh-desktop/ui
npx playwright install chrome
npm run test:ui
cd ../../..
cargo clippy -p oflh-desktop --all-targets --features desktop --locked -- -D warnings
cargo test -p oflh-desktop --features desktop --locked
npm --prefix crates/oflh-desktop/ui run tauri -- build --no-bundle -- --locked
cargo build --release --locked --bin oflh
```

The Desktop workflow gates packaging on frontend tests, then runs native Rust
regressions and builds packages on Linux, macOS and Windows, on x86-64 and ARM64.
A configured runner is not a completed validation: inspect all six job results
for the exact commit being distributed.

| Layer | Coverage | Boundary |
|---|---|---|
| Core / platform tests | Search, paths, process identities, native inspection and actions | Native coverage depends on the runner OS |
| Desktop Rust tests | Contracts, column predicates, paging, errors, owned child process actions and ports | Does not drive the WebView |
| Playwright | Virtualization, details, ancestry actions, default-cancel confirmation, refresh, retained port scope, column filter IPC, keyboard navigation, theme startup, resizing and preferences | Synthetic IPC; real DOM and frontend code |
| Packaged application | Native dialogs, drag-and-drop, clipboard, reveal, window startup and installation | Manual checks below remain required |

Browser failures retain screenshots and Playwright traces. Download the
`desktop-ui-report` workflow artifact and open its HTML report locally. Test
fixtures use synthetic names and paths; do not substitute private machine data
in screenshots committed to this repository.

## Native acceptance checklist

Record OS, architecture, app version and commit, package checksum, result, and
any failures for each target. Use disposable files and processes that you own.

- [ ] Install, launch, quit, relaunch and uninstall the exact generated package.
- [ ] First launch offers themes; later launches use the saved theme without a white flash.
- [ ] Light, dark and system themes, font size and details width persist.
- [ ] File/folder dialogs, pasted paths and drag-and-drop work, including spaces and Unicode.
- [ ] Inspect a fixture holding a file; verify its process, ancestry, handles and local ports.
- [ ] Search, per-column filters, sort, keyboard selection and copy return the expected data.
- [ ] F5 refresh preserves the current search and process scope without outlining the whole table.
- [ ] Terminate only an owned fixture; verify the result and that its filtered ports become empty.
- [ ] Cancel destructive dialogs; verify disappeared/stale processes produce understandable results without escalation.
- [ ] Reveal paths in the native file manager; check missing paths and permission failures.
- [ ] Resize the window, columns and details panel; test display scaling and keyboard-only operation.
- [ ] On Linux, check Wayland and X11; test the renderer workaround on affected NVIDIA systems.

The free direct [Tauri WebDriver tooling](https://v2.tauri.app/develop/tests/webdriver/)
supports Linux and Windows, but not macOS WKWebView. Native WebDriver automation
is not yet part of this repository's suite; browser coverage must not be reported
as native end-to-end coverage. Signing/notarization and installation acceptance
also remain distribution gates, not claims made by a successful build.

## README screenshots

Regenerate the Desktop preview from the synthetic Playwright fixture:

```sh
cd crates/oflh-desktop/ui
OFLH_UPDATE_SCREENSHOTS=1 npm run test:ui -- --grep 'documentation screenshot'
```

Review `images/desktop.png` after regeneration. The existing terminal demo stays
alongside it in the README. Screenshot generation is explicit; ordinary tests
write previews under ignored `test-results/` and never rewrite documentation.

## Packaging without publishing

Use the [release procedure](releasing.md) to collect Desktop packages separately
from CLI artifacts. Bundle the release executable already tested; do not silently
rebuild it between testing and packaging. Ensure both `--version` outputs and installer metadata match the release tag.
Local builds display development.
Local validation does not authorize a tag, upload, or published release.
