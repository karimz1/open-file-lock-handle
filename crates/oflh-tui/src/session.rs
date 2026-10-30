//! Owner and login session of a process, plus the guarded sign-out prompt.
//!
//! Lookups are lazy (only for the process on screen) and the session table is
//! cached briefly, so drawing never runs `loginctl` or WTS calls per frame.

use crate::view::messages::Language;
use oflh_sessions::{
    Action, Availability, Os, Privilege, Session, SessionKey, SessionKind, SessionState, Source,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const TTL: Duration = Duration::from_secs(3);

/// Read time, session table and PID-to-session cache.
type Table = (Instant, Vec<Session>, HashMap<u32, Option<String>>);

/// Session lookups for the TUI. Without a source nothing is shown.
#[derive(Default)]
pub struct Sessions {
    source: Option<Arc<dyn Source>>,
    pub os: Option<Os>,
    pub elevated: bool,
    table: Option<Table>,
}

impl Sessions {
    /// Real sessions of this computer.
    pub fn native(elevated: bool) -> Self {
        Self::new(Arc::from(oflh_sessions::native()), Os::current(), elevated)
    }
    pub fn new(source: Arc<dyn Source>, os: Os, elevated: bool) -> Self {
        Self {
            source: Some(source),
            os: Some(os),
            elevated,
            table: None,
        }
    }
    pub fn source(&self) -> Option<&Arc<dyn Source>> {
        self.source.as_ref()
    }
    fn fresh(&mut self) -> Option<&mut Table> {
        let source = self.source.as_ref()?;
        if self
            .table
            .as_ref()
            .is_none_or(|(at, ..)| at.elapsed() > TTL)
        {
            let sessions = source.sessions().unwrap_or_default();
            self.table = Some((Instant::now(), sessions, HashMap::new()));
        }
        self.table.as_mut()
    }
    /// The session a process runs in, when the OS has one for it.
    pub fn of(&mut self, pid: u32) -> Option<Session> {
        let source = self.source.clone()?;
        let (_, sessions, pids) = self.fresh()?;
        let id = pids
            .entry(pid)
            .or_insert_with(|| source.process_session(pid))
            .clone()?;
        sessions.iter().find(|session| session.id == id).cloned()
    }
    /// Whether `session` is the one this OFLH runs in.
    pub fn is_own(&self, session: &Session) -> bool {
        self.source
            .as_ref()
            .and_then(|source| source.own_session())
            .is_some_and(|own| own == session.id)
    }
    pub fn availability(&self, action: Action, session: &Session) -> Availability {
        match self.os {
            Some(os) => {
                oflh_sessions::availability(action, os, self.elevated, self.is_own(session))
            }
            None => Availability::Unsupported,
        }
    }
    /// Forget the cached table, e.g. after a sign-out.
    pub fn invalidate(&mut self) {
        self.table = None
    }
}

/// A pending, not yet confirmed sign-out.
#[derive(Clone, Debug)]
pub struct SignOutPrompt {
    pub session: Session,
    /// "name (PID n)" for every scanned process in the session.
    pub processes: Vec<String>,
    /// Focus is on the destructive button. Starts on Cancel.
    pub confirm: bool,
}
impl SignOutPrompt {
    pub fn key(&self) -> SessionKey {
        self.session.key()
    }
}

fn kind(language: Language, kind: SessionKind) -> &'static str {
    match (kind, language) {
        (SessionKind::Console, Language::German) => "Konsole",
        (SessionKind::Console, Language::Chinese) => "控制台",
        (SessionKind::Console, _) => "Console",
        (SessionKind::Rdp, _) => "RDP",
        (SessionKind::Ssh, _) => "SSH",
        (SessionKind::Tty, Language::Chinese) => "终端",
        (SessionKind::Tty, _) => "Terminal",
        (SessionKind::X11, _) => "X11",
        (SessionKind::Wayland, _) => "Wayland",
        (SessionKind::Services, Language::German) => "Dienste",
        (SessionKind::Services, Language::Chinese) => "服务",
        (SessionKind::Services, _) => "Services",
        (SessionKind::Other, Language::German) => "Andere",
        (SessionKind::Other, Language::Chinese) => "其他",
        (SessionKind::Other, _) => "Other",
    }
}

fn state(language: Language, state: SessionState) -> &'static str {
    match (state, language) {
        (SessionState::Active, Language::German) => "aktiv",
        (SessionState::Active, Language::Chinese) => "活动",
        (SessionState::Active, _) => "active",
        (SessionState::Idle, Language::German) => "inaktiv",
        (SessionState::Idle, Language::Chinese) => "空闲",
        (SessionState::Idle, _) => "idle",
        (SessionState::Disconnected, Language::German) => "getrennt",
        (SessionState::Disconnected, Language::Chinese) => "已断开",
        (SessionState::Disconnected, _) => "disconnected",
        (SessionState::Closing, Language::German) => "meldet ab",
        (SessionState::Closing, Language::Chinese) => "正在注销",
        (SessionState::Closing, _) => "signing out",
        (SessionState::Other, Language::German) => "Status unbekannt",
        (SessionState::Other, Language::Chinese) => "状态未知",
        (SessionState::Other, _) => "unknown state",
    }
}

/// "Session 3 (RDP)" or "SSH pts/2".
pub fn name(language: Language, session: &Session) -> String {
    let kind_name = kind(language, session.kind);
    match (&session.tty, session.kind) {
        (Some(tty), SessionKind::Ssh | SessionKind::Tty) => format!("{kind_name} {tty}"),
        _ => match language {
            Language::German => format!("Sitzung {} ({kind_name})", session.id),
            Language::Chinese => format!("会话 {}（{kind_name}）", session.id),
            Language::English => format!("Session {} ({kind_name})", session.id),
        },
    }
}

/// "2 h 05 min", relative to `now` (no time zone database in the TUI).
fn elapsed(since: u64, now: u64) -> Option<String> {
    let minutes = now.checked_sub(since)? / 60;
    Some(if minutes >= 60 {
        format!("{} h {:02} min", minutes / 60, minutes % 60)
    } else {
        format!("{minutes} min")
    })
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |time| time.as_secs())
}

/// One line such as `max.muster · Session 3 (RDP) · active for 2 h 05 min · from LAPTOP-7`.
pub fn owner_line(language: Language, user: &str, session: Option<&Session>, own: bool) -> String {
    owner_line_at(language, user, session, own, now())
}
pub fn owner_line_at(
    language: Language,
    user: &str,
    session: Option<&Session>,
    own: bool,
    now: u64,
) -> String {
    let mut parts = vec![if user.is_empty() {
        "?".into()
    } else {
        user.to_string()
    }];
    if oflh_sessions::is_service_account(user) {
        parts.push(
            match language {
                Language::German => "Dienstkonto",
                Language::Chinese => "服务账户",
                Language::English => "Service account",
            }
            .into(),
        );
    }
    if let Some(session) = session {
        parts.push(name(language, session));
        let status = state(language, session.state);
        parts.push(match session.since.and_then(|since| elapsed(since, now)) {
            Some(time) if session.state == SessionState::Active => match language {
                Language::German => format!("{status} seit {time}"),
                Language::Chinese => format!("{status} {time}"),
                Language::English => format!("{status} for {time}"),
            },
            _ => status.into(),
        });
        if let Some(client) = &session.client {
            parts.push(match language {
                Language::German => format!("von {client}"),
                Language::Chinese => format!("来自 {client}"),
                Language::English => format!("from {client}"),
            })
        }
        if own {
            parts.push(
                match language {
                    Language::German => "deine Sitzung",
                    Language::Chinese => "你的会话",
                    Language::English => "your session",
                }
                .into(),
            )
        }
    }
    oflh_core::safe(&parts.join(" · "))
}

/// Why an action is disabled, or `None` when it can run or does not exist here.
pub fn requirement(language: Language, availability: Availability) -> Option<&'static str> {
    match (availability, language) {
        (Availability::Requires(Privilege::Administrator), Language::German) => {
            Some("Erfordert Administratorrechte (starte OFLH als Admin)")
        }
        (Availability::Requires(Privilege::Administrator), Language::Chinese) => {
            Some("需要管理员权限（以管理员身份启动 OFLH）")
        }
        (Availability::Requires(Privilege::Administrator), _) => {
            Some("Requires administrator rights (restart OFLH as admin)")
        }
        (Availability::Requires(Privilege::Root), Language::German) => {
            Some("Erfordert root (starte OFLH mit sudo)")
        }
        (Availability::Requires(Privilege::Root), Language::Chinese) => {
            Some("需要 root（用 sudo 启动 OFLH）")
        }
        (Availability::Requires(Privilege::Root), _) => {
            Some("Requires root (start OFLH with sudo)")
        }
        _ => None,
    }
}

pub fn unsupported(language: Language) -> &'static str {
    match language {
        Language::German => "Abmelden ist auf diesem System nicht verfügbar.",
        Language::Chinese => "此系统不支持注销用户。",
        Language::English => "Signing users out isn't available on this system.",
    }
}

pub fn prompt_title(language: Language, user: &str) -> String {
    match language {
        Language::German => format!("{user} abmelden?"),
        Language::Chinese => format!("注销 {user}？"),
        Language::English => format!("Sign out {user}?"),
    }
}
pub fn prompt_note(language: Language, user: &str) -> String {
    match language {
        Language::German => format!(
            "Alle Programme in dieser Sitzung werden geschlossen. Ungespeicherte Arbeit von {user} geht verloren."
        ),
        Language::Chinese => format!("此会话中的所有程序都会关闭。{user} 未保存的工作将丢失。"),
        Language::English => {
            format!("Every program in this session closes. Unsaved work of {user} will be lost.")
        }
    }
}
pub fn prompt_processes(language: Language, n: usize) -> String {
    match language {
        Language::German => format!("Programme, die geschlossen werden ({n}):"),
        Language::Chinese => format!("将关闭的程序（{n}）："),
        Language::English => format!("Programs that will close ({n}):"),
    }
}
pub fn button(language: Language, user: &str) -> String {
    match language {
        Language::German => format!("{user} abmelden"),
        Language::Chinese => format!("注销 {user}"),
        Language::English => format!("Sign out {user}"),
    }
}
pub fn done(language: Language, user: &str) -> String {
    match language {
        Language::German => format!("{user} wird abgemeldet."),
        Language::Chinese => format!("正在注销 {user}。"),
        Language::English => format!("{user} is being signed out."),
    }
}
pub fn failed(language: Language, error: &oflh_sessions::Error) -> String {
    match (error, language) {
        (oflh_sessions::Error::Changed, Language::German) => {
            "Die Sitzung hat sich seit der Bestätigung geändert. Nichts wurde getan.".into()
        }
        (oflh_sessions::Error::Changed, Language::Chinese) => {
            "确认后会话已变化，未执行任何操作。".into()
        }
        (oflh_sessions::Error::Changed, _) => {
            "The session changed since you confirmed. Nothing was done.".into()
        }
        (error, Language::German) => format!("Abmelden fehlgeschlagen: {error}"),
        (error, Language::Chinese) => format!("注销失败：{error}"),
        (error, _) => format!("Sign-out failed: {error}"),
    }
}
pub fn hint(language: Language) -> &'static str {
    match language {
        Language::German => "L Benutzer abmelden",
        Language::Chinese => "L 注销用户",
        Language::English => "L sign out user",
    }
}
