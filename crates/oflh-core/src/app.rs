//! Which application a file holder belongs to.
//!
//! Multi-process apps keep files open in helper processes. Ending a helper
//! does not close the app: the main process starts a new helper, which opens
//! the file again. Closing the app means closing its main process.
//!
//! The rules here never look at names. A parent belongs to the same app only
//! when it runs the same executable (or, on macOS, the same `.app` bundle),
//! belongs to the same user and session, is not a service or critical
//! process, and is the visible app. Launchers, shells, terminals, file
//! managers and session managers run other executables, so the walk stops
//! before them.
use crate::{Identity, impact::ProcessEntry};
use std::path::{Component, Path, PathBuf};

/// The most ancestors followed from a holder.
pub const MAX_DEPTH: usize = 8;

/// How executables are compared on this platform.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Images {
    /// Exact path comparison (Linux).
    Exact,
    /// Case-insensitive path comparison (Windows).
    IgnoreCase,
    /// Same path, or both inside the same outermost `.app` bundle (macOS).
    Bundle,
}

impl Images {
    /// The rule for the operating system this was built for.
    pub const fn native() -> Self {
        if cfg!(windows) {
            Self::IgnoreCase
        } else if cfg!(target_os = "macos") {
            Self::Bundle
        } else {
            Self::Exact
        }
    }
}

/// One process on the way from a holder up through its parents, read just
/// before a confirmation is shown.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Lineage {
    /// Captured lifetime; a zero birth token is never followed.
    pub identity: Identity,
    /// Parent PID as reported by the OS.
    pub parent: u32,
    /// Untrusted OS name, only for display.
    pub name: String,
    /// Native executable path; empty when unreadable.
    pub executable: PathBuf,
    /// Owning account; `None` when unreadable.
    pub owner: Option<String>,
    /// Login or terminal session; `None` when the platform has none.
    pub session: Option<u32>,
    /// Managed by a service manager (systemd unit, Windows service, launchd job).
    pub service: bool,
    /// Ending it stops the system.
    pub critical: bool,
    /// The process is what a person sees as the app: it owns a visible
    /// top-level window, is the main executable of its macOS bundle, or
    /// shares the holder's desktop app scope on Linux.
    pub visible: bool,
}

/// The application that owns a helper process.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AppRoot {
    /// Captured lifetime of the app's main process.
    pub identity: Identity,
    /// Untrusted OS name of the main process.
    pub name: String,
    /// Native executable path of the main process.
    pub executable: PathBuf,
}

fn born(identity: Identity) -> (u64, u64) {
    (identity.started, identity.started_sub)
}

/// The outermost `X.app` directory that contains `path`.
pub fn outermost_bundle(path: &Path) -> Option<PathBuf> {
    let mut bundle = PathBuf::new();
    for component in path.components() {
        bundle.push(component);
        if let Component::Normal(name) = component
            && Path::new(name)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("app"))
        {
            return Some(bundle);
        }
    }
    None
}

/// Whether `candidate` runs the same app image as `holder`.
pub fn same_image(images: Images, holder: &Path, candidate: &Path) -> bool {
    if holder.as_os_str().is_empty() || candidate.as_os_str().is_empty() {
        return false;
    }
    match images {
        Images::Exact => holder == candidate,
        Images::IgnoreCase => holder
            .to_string_lossy()
            .eq_ignore_ascii_case(&candidate.to_string_lossy()),
        Images::Bundle => {
            holder == candidate
                || outermost_bundle(holder)
                    .is_some_and(|bundle| outermost_bundle(candidate).as_ref() == Some(&bundle))
        }
    }
}

/// Whether `executable` is the main executable of its outermost macOS
/// bundle (`X.app/Contents/MacOS/<file>`), the process macOS shows as the app.
pub fn bundle_main_executable(executable: &Path) -> bool {
    outermost_bundle(executable)
        .is_some_and(|bundle| executable.parent() == Some(&bundle.join("Contents").join("MacOS")))
}

/// The systemd scope a desktop started an app in (`app-<id>-<n>.scope`),
/// from `/proc/<pid>/cgroup`. Shared by every process of one app launch.
pub fn desktop_app_scope(cgroup: &str) -> Option<String> {
    let path = cgroup.lines().find_map(|line| line.strip_prefix("0::"))?;
    let unit = path.trim().rsplit('/').next()?;
    (unit.starts_with("app-") && unit.ends_with(".scope")).then(|| unit.to_string())
}

/// Index in `chain` of the app that owns `chain[0]`, the holder.
///
/// `chain[0]` is the holder and `chain[i + 1]` the parent of `chain[i]`.
/// Returns `None` when the holder is itself the app or no parent passes
/// every rule; closing then targets the holder alone, as before.
pub fn app_root(chain: &[Lineage], images: Images) -> Option<usize> {
    let holder = chain.first()?;
    if holder.visible || holder.owner.is_none() {
        return None;
    }
    let mut top = 0;
    for index in 1..chain.len().min(MAX_DEPTH + 1) {
        let (child, parent) = (&chain[index - 1], &chain[index]);
        let same_app = parent.identity.started != 0
            && child.parent == parent.identity.pid
            && parent.identity.pid != child.identity.pid
            && born(parent.identity) <= born(child.identity)
            && same_image(images, &holder.executable, &parent.executable)
            && parent.owner == holder.owner
            && parent.session == holder.session
            && !parent.service
            && !parent.critical;
        if !same_app {
            break;
        }
        top = index;
    }
    (1..=top).rev().find(|&index| chain[index].visible)
}

/// Every running descendant of `root`, at any depth, oldest first.
///
/// Uses the same PID-reuse guard as [`crate::impact::children_of`]: a child
/// must not be older than its parent.
pub fn descendants_of(table: &[ProcessEntry], root: Identity) -> Vec<Identity> {
    let mut found: Vec<Identity> = Vec::new();
    let mut parents = vec![root];
    while let Some(parent) = parents.pop() {
        for child in crate::impact::children_of(table, parent) {
            if child.identity != root && !found.contains(&child.identity) {
                found.push(child.identity);
                parents.push(child.identity);
            }
        }
    }
    found.sort_by_key(|identity| (born(*identity), identity.pid));
    found
}

#[cfg(test)]
mod tests;
