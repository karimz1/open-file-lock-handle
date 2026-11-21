# Desktop GUI end-to-end tests

These tests drive the real OFLH desktop app, not a mock. A debug build with the
test-only `gui-e2e` Cargo feature embeds a local W3C WebDriver server
([tauri-plugin-wdio-webdriver](https://github.com/webdriverio/desktop-mobile)).
[WebdriverIO](https://webdriver.io) connects to it, clicks, types and reads the
window, and checks the result against real processes started by the tests: Node
helpers that hold files open or listen on a TCP port, and on Windows a WinForms
window for the normal-close path.

The browser tests in `ui/tests` stay the fast check for UI logic against a mocked
backend. This suite checks that the pieces work together on each operating
system: scanning, the clipboard, the file manager, waits and terminating
processes.

The embedded server is the approach the Tauri documentation recommends for
WebDriver testing, and the only free one that also covers macOS (WKWebView has
no standalone driver). `tests/gui_e2e_feature.rs` makes sure no shipping build
enables the feature.

## Run it

```sh
(cd ../ui && npm ci && npm run tauri -- build --debug --no-bundle --features gui-e2e -- --locked)
npm ci
npm test                      # Windows and macOS
xvfb-run -a -s '-screen 0 1600x1000x24' dbus-run-session -- ./run-linux.sh   # Linux
```

Linux also needs `xvfb openbox xclip python3-dbus python3-gi`. Run one file with
`--spec specs/04-details-panel.e2e.ts`. `OFLH_E2E_APP` points at another build;
`OFLH_E2E_ARTIFACTS` changes where screenshots, app logs and the summary go
(default `artifacts/<os>-<arch>/`).

## Layout

| Path                                        | What it holds                                                                                |
| ------------------------------------------- | -------------------------------------------------------------------------------------------- |
| `specs/01-first-launch-and-settings.e2e.ts` | Theme chooser, Settings, theme, zoom, language, About, Donate, sidebar                       |
| `specs/02-inspect-and-views.e2e.ts`         | Inspect a path, the four views, shortcuts, refresh, close, recent targets                    |
| `specs/03-search-and-selection.e2e.ts`      | Search, row selection, select all, copy to clipboard                                         |
| `specs/04-details-panel.e2e.ts`             | Process details: copy, reveal, ancestry, waits, owner folder                                 |
| `specs/05-termination.e2e.ts`               | Close and force quit, confirmation, cancel, results                                          |
| `specs/06-grid-keyboard-and-layout.e2e.ts`  | Sorting, columns, filters, maximize, keyboard navigation, virtual scroll, auto refresh       |
| `specs/07-ports.e2e.ts`                     | Ports view, port search, copy port, wait for a port                                          |
| `support/`                                  | App launch, fixture processes, desktop checks (clipboard, file manager), UI helpers, results |

## Results and screenshots

Every test ends with a screenshot, and some take more at key steps. After the
run, `summary.md` lists each test as passed, failed, known failing or not
applicable. In CI it is added to the job summary, and the
[GUI E2E workflow](../../../.github/workflows/gui-e2e.yml) uploads
`gui-e2e-<os>-<arch>` for three days. Once every platform of a run has uploaded,
the workflow deletes the same artifacts from older runs of that branch or pull
request, so only the newest commit keeps screenshots.

## Known failing and platform-only tests

- `itKnownFailing(issue, title, fn)` marks a reproduced bug with an open issue.
  It passes while the bug is there and fails as soon as the test passes, with a
  reminder to turn it back into `it`. Nothing is skipped silently. Currently
  marked: #186 (search and keyboard focus), #188 (language switch leaves
  Settings), and on Windows only #201 (working directory is Unavailable
  because the backend never fills `Process.cwd`).
- `itOn(systems, reason, title, fn)` runs only where the behavior exists (for
  example the Windows normal-close refusal) and reports "not applicable" with the
  reason elsewhere.

## What WebDriver cannot reach

The embedded server dispatches synthetic DOM events, so these stay with the
mocked browser tests or manual testing:

- native file and folder pickers (Open file, Open folder, Choose file)
- drag and drop from the file manager
- New window, which starts a second app instance
- links that open the web browser (GitHub, Donate destinations)
- browser-native key handling: Escape on a native dialog, Enter submitting a
  form, Tab order, option changes in a native select (the tests set the value
  and dispatch `change`)
- the Windows Explorer context menu and the installer (see `tests/installer`)
