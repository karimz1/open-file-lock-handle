# Contributing to oflh

Bug reports, documentation fixes, translations, and focused code changes are
all welcome.

## Report a bug

Search [existing issues](https://github.com/karimz1/open-file-lock-handle/issues)
first. A good report includes:

- The output of `oflh --version`, or the details copied from **About OFLH** in
  the desktop app.
- Your OS, version, and CPU architecture.
- Terminal or desktop app, and for the terminal, which terminal emulator.
- What you inspected (file, folder, drive root, or port), what you expected,
  and what you saw.
- Any warning oflh showed. Press `?` in the terminal app or open **coverage
  notices** in the desktop app.

Reproduce with temporary files where you can. Before posting, remove user
names, private paths, host names, process command lines, and credentials from
logs and screenshots.

## Submit a change

1. Build and run the project with [Development](docs/development.md).
2. Keep the change focused, and put code in the crate that owns it
   ([Architecture](docs/architecture.md)).
3. Add a regression test for any behavior change. For terminal layout changes,
   update the golden snapshots and include before/after screenshots that use
   synthetic process names and paths.
4. Run `cargo xtask check`. For desktop changes, also run the frontend checks
   listed in [Development](docs/development.md#validate-changes).
5. Review your diff for generated files and personal data.
6. Open a pull request that explains the problem, the new behavior, and how
   you tested it.

## Code expectations

- Standard `rustfmt` formatting, descriptive names, small functions.
- Typed errors (`thiserror`) that keep the operation and the OS error code; no
  `unwrap()` or `expect()` in production code.
- Safe Rust outside `oflh-platform`. Every `unsafe` block there has a
  `// SAFETY:` comment explaining the ABI or lifetime assumption.
- Never weaken process-identity checks, protected-process guards, or
  default-cancel confirmations to simplify a change.
- Keep the distinction between "file is open" and "file is locked".

Changes to a platform backend need to pass on the affected operating systems
and both architectures; CI runs all six for every pull request.
Cross-compiling does not test OS behavior. Performance claims need
reproducible numbers with identical coverage; see
[Inspection performance](docs/inspection-performance.md).

Translations: see the
[desktop translation guide](crates/oflh-desktop/ui/src/locales/README.md) and
the terminal notes in [Development](docs/development.md#terminal-snapshots-and-translations).
