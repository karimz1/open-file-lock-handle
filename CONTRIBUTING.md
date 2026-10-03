# Contributing

Bug reports, documentation fixes, and focused code changes are welcome.

## Report a bug

Search [existing issues](https://github.com/karimz1/open-file-lock-handle/issues)
before opening a new one. Include:

- App version (`oflh --version` or Desktop Settings), operating system, and architecture.
- Which interface you used; include the terminal application for CLI issues.
- Steps to reproduce, expected behavior, and what happened instead.
- Relevant warnings shown by `oflh` and whether the target is a file, folder, or port.

Use a minimal example with temporary files where possible. Redact usernames,
private paths, hostnames, process arguments, and credentials from screenshots or
logs. Do not post sensitive data in a public issue.

## Make a change

1. Follow [Development](docs/development.md) to build and run the project.
2. Keep the change focused and follow the boundaries in [Architecture](docs/architecture.md).
3. Add regression coverage for behavior changes. Include before/after screenshots
   for terminal layout changes, using synthetic process names and paths.
4. Run `cargo xtask check`. For Desktop changes, follow the checks in the
   [Desktop CI workflow](.github/workflows/desktop.yml).
   Review your diff for generated files and sensitive data.
5. Open a pull request explaining the problem, resulting behavior, and validation.

Use standard Rust formatting and descriptive names. Prefer small functions,
typed errors, owned resources, and safe Rust. Document public APIs and explain
native ABI or lifetime assumptions beside each unsafe block. Do not weaken
identity checks or confirmation behavior to simplify a change.

Native changes need tests on the affected operating systems and architectures;
cross-compilation alone cannot validate operating-system behavior. Performance
changes should include reproducible measurements using the same workload and
inspection coverage.
