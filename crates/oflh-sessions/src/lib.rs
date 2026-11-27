//! Who owns a process and which login session it runs in, plus the session
//! and network-share actions a terminal-server administrator needs.
//!
//! Everything here is informational until an action is requested. Actions
//! take a [`SessionKey`] or [`ShareFileKey`] captured when the user confirmed
//! and are re-validated against a fresh native read right before they run,
//! so a session that was replaced (same id, different user or logon time) or
//! a share handle that now points at another file is never acted on.
#![deny(missing_docs)]
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

use std::{fmt, path::Path};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(any(target_os = "linux", test))]
pub mod loginctl;
#[cfg(windows)]
mod windows;
pub mod wts;

/// Operating systems with different session models.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Os {
    /// Remote Desktop Services sessions and SMB server open files.
    Windows,
    /// systemd-logind sessions.
    Linux,
    /// User only; no session actions.
    MacOs,
    /// Anything else: no session information.
    Other,
}

impl Os {
    /// The operating system this binary was built for.
    pub const fn current() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else if cfg!(target_os = "linux") {
            Self::Linux
        } else if cfg!(target_os = "macos") {
            Self::MacOs
        } else {
            Self::Other
        }
    }
}

/// How a session was opened.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub enum SessionKind {
    /// Local console (Windows console session, Linux seat).
    Console,
    /// Remote Desktop (RDP) or xrdp.
    Rdp,
    /// SSH login.
    Ssh,
    /// Text terminal login.
    Tty,
    /// Graphical X11 session.
    X11,
    /// Graphical Wayland session.
    Wayland,
    /// Windows session 0 or a non-interactive logind session (cron, systemd user).
    Services,
    /// Unknown or not reported.
    #[default]
    Other,
}

impl SessionKind {
    /// Stable machine name shared with the desktop UI.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Console => "console",
            Self::Rdp => "rdp",
            Self::Ssh => "ssh",
            Self::Tty => "tty",
            Self::X11 => "x11",
            Self::Wayland => "wayland",
            Self::Services => "services",
            Self::Other => "other",
        }
    }
}

/// Whether someone is connected to the session.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub enum SessionState {
    /// Someone is connected and using it.
    Active,
    /// Connected but idle or locked / in the background.
    Idle,
    /// Nobody is connected; programs keep running.
    Disconnected,
    /// Being torn down.
    Closing,
    /// Unknown or not reported.
    #[default]
    Other,
}

impl SessionState {
    /// Stable machine name shared with the desktop UI.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Idle => "idle",
            Self::Disconnected => "disconnected",
            Self::Closing => "closing",
            Self::Other => "other",
        }
    }
}

/// One login session as reported by the OS.
#[derive(Clone, Debug, Default, Eq, PartialEq, Hash)]
pub struct Session {
    /// Native session id: a WTS session number or a logind session name.
    pub id: String,
    /// Account name without domain; empty when nobody is signed in.
    pub user: String,
    /// Windows domain or computer name.
    pub domain: Option<String>,
    /// How the session was opened.
    pub kind: SessionKind,
    /// Connection state.
    pub state: SessionState,
    /// Remote client computer or host, for RDP/SSH/xrdp.
    pub client: Option<String>,
    /// Terminal, such as `pts/2` or `tty3`.
    pub tty: Option<String>,
    /// Logon (or connect) time in seconds since the Unix epoch.
    pub since: Option<u64>,
}

impl Session {
    /// The identity an action is bound to.
    pub fn key(&self) -> SessionKey {
        SessionKey {
            id: self.id.clone(),
            user: self.user.clone(),
            since: self.since,
        }
    }
}

/// What a confirmed session action is bound to. A session whose id was
/// reused for another sign-in no longer matches.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct SessionKey {
    /// Native session id.
    pub id: String,
    /// Account signed in to it when confirmed.
    pub user: String,
    /// Logon time when confirmed.
    pub since: Option<u64>,
}

/// A file on this computer opened by someone over an SMB share.
#[derive(Clone, Debug, Default, Eq, PartialEq, Hash)]
pub struct ShareFile {
    /// Server-side open-file id (`NetFileEnum` `fi3_id`).
    pub id: u32,
    /// Local path of the opened file.
    pub path: String,
    /// Account that opened it.
    pub user: String,
    /// Client computers this account is connected from, when known.
    pub client: Option<String>,
    /// Number of byte-range locks held.
    pub locks: u32,
    /// Whether the file is open for writing.
    pub write: bool,
}

impl ShareFile {
    /// The identity a "Close open file" action is bound to.
    pub fn key(&self) -> ShareFileKey {
        ShareFileKey {
            id: self.id,
            path: self.path.clone(),
            user: self.user.clone(),
        }
    }
    /// Whether this open is the inspected file, or inside the inspected folder.
    /// Windows paths compare case-insensitively with either slash.
    pub fn matches(&self, target: &Path, directory: bool) -> bool {
        // NetFileEnum reports plain `C:\dir\file` paths, while an inspected
        // target is canonicalized into the verbatim `\\?\C:\dir\file` form.
        let norm = |value: &str| {
            let value = value.replace('/', "\\");
            let value = if let Some(rest) = value
                .strip_prefix(r"\\?\UNC\")
                .or_else(|| value.strip_prefix(r"\\?\unc\"))
            {
                format!(r"\\{rest}")
            } else {
                value
                    .strip_prefix(r"\\?\")
                    .or_else(|| value.strip_prefix(r"\??\"))
                    .unwrap_or(&value)
                    .to_owned()
            };
            value.trim_end_matches('\\').to_lowercase()
        };
        let open = norm(&self.path);
        let target = norm(&target.to_string_lossy());
        if target.is_empty() {
            return false;
        }
        open == target || (directory && open.starts_with(&format!("{target}\\")))
    }
}

/// What a confirmed "Close open file" action is bound to.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct ShareFileKey {
    /// Server open-file id.
    pub id: u32,
    /// Path it pointed at when confirmed.
    pub path: String,
    /// Account it belonged to when confirmed.
    pub user: String,
}

/// Errors from session reads and actions.
#[derive(Debug, Eq, PartialEq)]
pub enum Error {
    /// This platform or build has no such capability.
    Unsupported,
    /// The action needs administrator rights (or root).
    PermissionDenied,
    /// The session or open file changed or ended since it was confirmed.
    Changed,
    /// A native call or helper failed.
    Failed(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported => f.write_str("not supported on this system"),
            Self::PermissionDenied => f.write_str("requires administrator rights"),
            Self::Changed => f.write_str("the session or file changed since you confirmed"),
            Self::Failed(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for Error {}

/// Result alias.
pub type Result<T> = std::result::Result<T, Error>;

/// Actions offered for a session or a share file.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Action {
    /// Pop up a message box in the session.
    SendMessage,
    /// Sign the user out of the session.
    SignOut,
    /// Close a file opened over an SMB share.
    CloseShareFile,
}

/// Elevation an action needs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Privilege {
    /// Windows administrator.
    Administrator,
    /// Unix root.
    Root,
}

/// Whether an action can be offered, and why not.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Availability {
    /// Can be run now.
    Available,
    /// Shown disabled: the user must restart OFLH with this privilege.
    /// OFLH never elevates on its own for session actions.
    Requires(Privilege),
    /// Not offered on this platform.
    Unsupported,
}

impl Availability {
    /// Stable machine name shared with the desktop UI.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::Requires(Privilege::Administrator) => "requires_admin",
            Self::Requires(Privilege::Root) => "requires_root",
            Self::Unsupported => "unsupported",
        }
    }
}

/// Decide whether `action` can be offered. Messages and sign-out of another
/// user's session need elevation; the own session never does on Windows.
/// Linux sign-out always asks for root so the rule stays predictable.
pub fn availability(action: Action, os: Os, elevated: bool, own_session: bool) -> Availability {
    let admin = |needed: bool, privilege| {
        if needed && !elevated {
            Availability::Requires(privilege)
        } else {
            Availability::Available
        }
    };
    match (os, action) {
        (Os::Windows, Action::SendMessage | Action::SignOut) => {
            admin(!own_session, Privilege::Administrator)
        }
        (Os::Windows, Action::CloseShareFile) => admin(true, Privilege::Administrator),
        (Os::Linux, Action::SignOut) => admin(true, Privilege::Root),
        _ => Availability::Unsupported,
    }
}

/// Native (or fake) source of sessions and share files.
pub trait Source: Send + Sync {
    /// All current sessions. Empty when the platform has none.
    fn sessions(&self) -> Result<Vec<Session>>;
    /// The session id a process runs in, or `None` when unknown.
    fn process_session(&self, pid: u32) -> Option<String>;
    /// The session this program runs in.
    fn own_session(&self) -> Option<String> {
        self.process_session(std::process::id())
    }
    /// Files opened over SMB shares on this computer.
    fn share_files(&self) -> Result<Vec<ShareFile>> {
        Err(Error::Unsupported)
    }
    /// Show a message box in a session. Caller has re-validated the key.
    fn send_message(&self, _session: &str, _title: &str, _text: &str) -> Result<()> {
        Err(Error::Unsupported)
    }
    /// Sign the session's user out. Caller has re-validated the key.
    fn sign_out(&self, _session: &str) -> Result<()> {
        Err(Error::Unsupported)
    }
    /// Close an open share file. Caller has re-validated the key.
    fn close_share_file(&self, _id: u32) -> Result<()> {
        Err(Error::Unsupported)
    }
}

/// Re-read sessions and require that the captured one still exists with the
/// same user and logon time. Returns the fresh record.
pub fn revalidate(source: &dyn Source, key: &SessionKey) -> Result<Session> {
    source
        .sessions()?
        .into_iter()
        .find(|session| session.key() == *key)
        .ok_or(Error::Changed)
}

/// Re-read share files and require that the open id still names the same path and user.
pub fn revalidate_share_file(source: &dyn Source, key: &ShareFileKey) -> Result<ShareFile> {
    source
        .share_files()?
        .into_iter()
        .find(|file| file.key() == *key)
        .ok_or(Error::Changed)
}

/// Sign out a captured session after re-validating it.
pub fn sign_out(source: &dyn Source, key: &SessionKey) -> Result<()> {
    let session = revalidate(source, key)?;
    source.sign_out(&session.id)
}

/// Send a message to a captured session after re-validating it.
pub fn send_message(source: &dyn Source, key: &SessionKey, title: &str, text: &str) -> Result<()> {
    let session = revalidate(source, key)?;
    let text = text.trim();
    if text.is_empty() || text.chars().count() > 1000 {
        return Err(Error::Failed("message must be 1 to 1000 characters".into()));
    }
    source.send_message(&session.id, title, text)
}

/// Close a captured share open after re-validating it.
pub fn close_share_file(source: &dyn Source, key: &ShareFileKey) -> Result<()> {
    let file = revalidate_share_file(source, key)?;
    source.close_share_file(file.id)
}

/// Accounts that run services rather than people.
pub fn is_service_account(user: &str) -> bool {
    let bare = user.rsplit(['\\', '/']).next().unwrap_or(user);
    let lower = bare.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "system"
            | "local service"
            | "localservice"
            | "network service"
            | "networkservice"
            | "trustedinstaller"
            | "root"
            | "daemon"
            | "nobody"
            | "_windowserver"
            | "messagebus"
            | "systemd-network"
            | "systemd-resolve"
            | "systemd-timesync"
            | "polkitd"
            | "avahi"
            | "colord"
            | "gdm"
    ) || lower.starts_with("dwm-")
        || lower.starts_with("umfd-")
        || (lower.starts_with('_') && cfg!(target_os = "macos"))
        || lower.starts_with("nt service\\")
        || user.to_ascii_lowercase().starts_with("nt service\\")
        || user.to_ascii_lowercase().starts_with("nt authority\\")
}

/// The source for this operating system. macOS and others report no sessions.
pub fn native() -> Box<dyn Source> {
    #[cfg(target_os = "linux")]
    {
        Box::new(linux::Native)
    }
    #[cfg(windows)]
    {
        Box::new(windows::Native)
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        Box::new(Empty)
    }
}

/// A source with no sessions and no actions (macOS, unsupported systems).
#[derive(Clone, Copy, Debug, Default)]
pub struct Empty;

impl Source for Empty {
    fn sessions(&self) -> Result<Vec<Session>> {
        Ok(Vec::new())
    }
    fn process_session(&self, _: u32) -> Option<String> {
        None
    }
}

/// In-memory source for tests and UI fixtures. Records every action.
#[derive(Debug, Default)]
pub struct Fake {
    /// Sessions returned by [`Source::sessions`].
    pub sessions: std::sync::Mutex<Vec<Session>>,
    /// PID to session id.
    pub processes: std::sync::Mutex<std::collections::HashMap<u32, String>>,
    /// The caller's own session.
    pub own: Option<String>,
    /// Share files returned by [`Source::share_files`].
    pub share_files: std::sync::Mutex<Vec<ShareFile>>,
    /// Actions performed, as `"sign_out 3"`, `"message 3 text"`, `"close 7"`.
    pub log: std::sync::Mutex<Vec<String>>,
    /// Error every action returns, when set.
    pub fail: Option<Error>,
}

fn lock<T>(mutex: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|error| error.into_inner())
}

impl Fake {
    fn act(&self, entry: String) -> Result<()> {
        if let Some(error) = &self.fail {
            return Err(match error {
                Error::Unsupported => Error::Unsupported,
                Error::PermissionDenied => Error::PermissionDenied,
                Error::Changed => Error::Changed,
                Error::Failed(message) => Error::Failed(message.clone()),
            });
        }
        lock(&self.log).push(entry);
        Ok(())
    }
    /// Recorded actions.
    pub fn actions(&self) -> Vec<String> {
        lock(&self.log).clone()
    }
}

impl Source for Fake {
    fn sessions(&self) -> Result<Vec<Session>> {
        Ok(lock(&self.sessions).clone())
    }
    fn process_session(&self, pid: u32) -> Option<String> {
        lock(&self.processes).get(&pid).cloned()
    }
    fn own_session(&self) -> Option<String> {
        self.own.clone()
    }
    fn share_files(&self) -> Result<Vec<ShareFile>> {
        Ok(lock(&self.share_files).clone())
    }
    fn send_message(&self, session: &str, _: &str, text: &str) -> Result<()> {
        self.act(format!("message {session} {text}"))
    }
    fn sign_out(&self, session: &str) -> Result<()> {
        self.act(format!("sign_out {session}"))?;
        lock(&self.sessions).retain(|item| item.id != session);
        Ok(())
    }
    fn close_share_file(&self, id: u32) -> Result<()> {
        self.act(format!("close {id}"))?;
        lock(&self.share_files).retain(|item| item.id != id);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
