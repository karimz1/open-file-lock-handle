//! Windows 11 File Explorer context menu handler for OFLH Desktop.
//!
//! Windows 11 shows only `IExplorerCommand` handlers that come from a package
//! with identity in its top-level context menu. This crate builds that handler
//! as a small in-process COM DLL. The identity package ("package with external
//! location", `package/AppxManifest.xml`) declares it for files, folders and the
//! folder background and points at the regular OFLH Desktop install folder, so
//! the classic NSIS installer stays the only installer.
//!
//! The handler does one thing: it starts `oflh-desktop.exe --inspect <path>`,
//! the same command the classic registry verbs run. Everything that decides
//! what is shown or launched lives in [`command`] and is plain, tested Rust;
//! the COM glue in `com` (Windows only) stays as thin as possible.
//!
//! This is the second crate besides `oflh-platform` that needs `unsafe`: COM
//! exports (`DllGetClassObject`) and raw interface pointers cannot be written
//! without it. Every unsafe block is documented.

pub mod command;

#[cfg(windows)]
mod com;
