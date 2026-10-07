# AGENTS.md

Instructions for coding agents working in this repository. Human contributors
should read [CONTRIBUTING.md](CONTRIBUTING.md) first; everything here applies to
them too.

## Project in one paragraph

oflh (Open File Lock Handle) shows which processes use a file, folder, or local
port on Linux, macOS, and Windows (x86-64 and ARM64). It ships two frontends
over one Rust scanner: `oflh`, an interactive terminal app (ratatui), and
`oflh-desktop`, a Tauri 2 app with a React/TypeScript UI. It finds file users
by enumerating process references (descriptors, handles, mappings, modules),
not by walking directories. Licensed MIT.

## Commands

Run from the repository root. The toolchain is pinned in `rust-toolchain.toml`
(rustup installs it automatically).

| Task | Command |
| --- | --- |
| Build the shipped CLI | `cargo build --release --locked --bin oflh` |
| Run the CLI | `cargo run -- <path>` (needs a real TTY) |
| All Rust gates (required before handoff) | `cargo xtask check` |
| Format only | `cargo fmt --all` |
| One crate's tests | `cargo test -p oflh-core --locked` |
| Regenerate terminal snapshots | `OFLH_UPDATE_SNAPSHOTS=1 cargo test -p oflh-tui --locked` |
| Desktop frontend install | `npm --prefix crates/oflh-desktop/ui ci` |
| Desktop frontend checks | `npm --prefix crates/oflh-desktop/ui run format:check`, `... test`, `... run build` |
| Desktop browser tests | `npm --prefix crates/oflh-desktop/ui run test:ui` |
| Desktop app build | `npm --prefix crates/oflh-desktop/ui run tauri -- build --no-bundle -- --locked` |
| Updater tests | `cargo test -p oflh-desktop --features updater-tests --locked` |

`cargo xtask check` runs `cargo fmt --all --check`, then
`cargo clippy --workspace --all-targets --locked -- -D warnings`, then
`cargo test --workspace --locked`. It does not need WebKitGTK or Node.js: the
Tauri shell is behind the `oflh-desktop` crate's `desktop` feature.

## Layout

| Path | Contents | Rules |
| --- | --- | --- |
| `crates/oflh-core` | Targets, identities, observations, search, version rules | `#![forbid(unsafe_code)]`, no OS calls |
| `crates/oflh-platform` | Native backends (`linux.rs`, `macos.rs`, `windows/`), ports, metrics, termination, elevation, update HTTP | The only crate allowed `unsafe` |
| `crates/oflh-tui` | Terminal state (`app.rs`), rendering (`view.rs`), workers, strings (`messages.rs`) | `#![forbid(unsafe_code)]` |
| `crates/oflh` | `oflh` binary: argument parsing (`main.rs`), CLI help (`messages.rs`) | `#![forbid(unsafe_code)]` |
| `crates/oflh-desktop` | Desktop service (`src/`), Tauri config, React UI (`ui/src`), locales (`ui/src/locales`) | `#![forbid(unsafe_code)]` |
| `xtask` | `check`, packaging, release assembly, updater signing | Never shipped |
| `docs/` | User and contributor guides; `docs/measurements/` holds raw benchmark JSON | See documentation rules below |
| `.github/workflows` | `ci.yml` (six native runners), `desktop.yml`, `release.yml`, `homebrew.yml`, `updater-website.yml` | |

Details: [docs/architecture.md](docs/architecture.md),
[docs/development.md](docs/development.md).

## Things that surprise people

- The CLI has no non-interactive mode. It exits with status 1 when stdin or
  stdout is not a terminal, so you cannot capture its output with a pipe. Test
  UI behavior through `crates/oflh-tui` unit tests, golden snapshots, or the
  PTY tests in `crates/oflh/tests/terminal.rs`.
- Options must come before the path; a second positional argument is an error
  (exit status 2).
- Native tests start real processes that hold files, locks, and sockets, and
  compile a C fixture with the `cc` crate. They can fail in sandboxes that
  restrict `/proc`, netlink, or PTYs. That is an environment limitation to
  report, not a reason to change the test.
- Behaviour differs per OS. A change that passes on Linux can still break macOS
  or Windows. CI on the six native runners is the source of truth; say which
  targets you could not run.
- Profiling harnesses in `examples/` and the `native-query-experiment` and
  `profiling` features must never be linked into shipped binaries.

## Rust conventions

- Standard `rustfmt`; descriptive `snake_case` names. Single letters only for
  conventional loop indices and short, obvious closure parameters.
- Keep functions focused. Extract parsing, native resource management,
  observation collection, and UI state transitions into named helpers.
- Document public types and APIs with `///`, including non-obvious search,
  identity, cancellation, and native ABI invariants.
- Errors: typed with `thiserror`, carrying operation context and the original OS
  error code; propagate with `?`.
- No `unwrap()` or `expect()` in production code unless failure is statically
  impossible and the reason is written down. Tests may use them.
- Optimize by measurably reducing system calls, allocations, copies, and idle
  work. Do not claim a speedup without reproducible measurements at equal
  inspection coverage.

## Safety invariants (do not weaken)

- Confine `unsafe` to `oflh-platform`. Every block gets a `// SAFETY:` comment;
  verify buffer lengths and layouts before reading. The workspace denies
  `clippy::undocumented_unsafe_blocks` and `unsafe_op_in_unsafe_fn`.
- Use owned handles and descriptors with RAII. Keep native paths lossless;
  sanitize control and formatting characters only for display.
- Bind actions and metrics to PID plus birth identity. Keep stale-identity
  checks, protected-process guards (`Identity::validate`), default-cancel
  confirmations, and disclosure of hidden selections.
- An open file is not proof of a lock. Keep each platform's evidence limits,
  unknown metrics, and partial-result warnings. Windows resource users are not
  proven lock owners.
- Keep scanning off the UI thread, queues bounded, cancellation cooperative,
  and stale generations rejected. No periodic redraws while idle.
- Keep the existing keyboard workflow. A focused ancestry tree keeps its
  captured identities across refreshes.

## Tests

- Add a regression test for every behavior change or fixed bug. Unit tests go
  in `#[cfg(test)]` modules; integration tests under `tests/`.
- Golden snapshots in `crates/oflh-tui/tests/snapshots/` change only for
  intentional UI changes. Regenerate with `OFLH_UPDATE_SNAPSHOTS=1` and review
  every changed file.
- Keep the independent C lock fixture (`crates/oflh-platform/tests/fixtures/lock-fixture.c`).
- Platform backend changes need native validation on Linux, macOS, and Windows
  for both x86-64 and ARM64. Report unverified targets explicitly.
- When a check fails, find the cause. Never disable a test or relax a safety
  contract to make CI green.
- [docs/inspection-regressions.md](docs/inspection-regressions.md) maps each
  scanner contract to its tests.

## Documentation rules

- `README.md` is the landing page: what oflh does, when to use it, install,
  quick start, essential keys, limitations, FAQ. Reference material lives in
  `docs/`: terminal behavior in `docs/terminal-usage.md`, desktop in
  `docs/desktop-usage.md`, per-OS detection limits in
  `docs/platform-support.md`. Link rather than duplicate.
- Verify every command, flag, shortcut, label, and output sample against the
  code (`crates/oflh/src/main.rs`, `crates/oflh-tui/src/app.rs`,
  `crates/oflh-desktop/ui/src/App.tsx`, the snapshot files). Verify download
  names against the latest release's assets.
- Keep "open" vs "locked", Windows owner uncertainty, permission limits, and
  termination consequences next to the claims they qualify.
- Benchmarks: quote only numbers present in `docs/measurements/`, with their
  context (single run, which machine).
- Preserve existing heading anchors, or add an `<a id="...">` for the old one.
  The website and older links point at README anchors such as `#install`,
  `#getting-started`, `#desktop-install`, and `#cli-tui-install`.
- Write plainly: no marketing adjectives, no unsupported superlatives, no
  keyword stuffing. Check relative links and Markdown formatting before
  handing off.
- `llms.txt` at the repository root is a link index for tools; update it when
  adding or renaming docs.

## Scope and releases

- Do not push, tag, publish releases, or change GitHub settings unless the user
  asked for that specific action. A request for a local release candidate does
  not authorize pushing a tag.
- Release packaging and checksums stay in `xtask`. Release automation consumes
  the six tested native artifacts, rejects missing or unexpected ones, and never
  overwrites a published release. See [docs/releasing.md](docs/releasing.md).
- Preserve unrelated work. Report what you changed, how you validated it, and
  what you could not verify.

## Privacy

- Never commit credentials, tokens, private keys, personal contact details,
  private paths, or host names, in code, fixtures, screenshots, logs, or
  benchmark data.
- Use synthetic data in examples and fixtures (the snapshots use `/build`,
  `dotnet`, PID 424242, user `alice`).
- If you find sensitive data, report its category and location without
  repeating the value. Do not rewrite shared history or rotate credentials
  without authorization.
