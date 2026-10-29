//! Platform-neutral mapping of raw Remote Desktop Services (WTS) data, so it
//! can be unit-tested with fakes on any OS. The Windows backend only fills
//! [`RawSession`] from `WTSQuerySessionInformationW`.
use crate::{Session, SessionKind, SessionState};

/// `WTS_CONNECTSTATE_CLASS` values.
pub mod state {
    /// WTSActive
    pub const ACTIVE: i32 = 0;
    /// WTSConnected
    pub const CONNECTED: i32 = 1;
    /// WTSConnectQuery
    pub const CONNECT_QUERY: i32 = 2;
    /// WTSShadow
    pub const SHADOW: i32 = 3;
    /// WTSDisconnected
    pub const DISCONNECTED: i32 = 4;
    /// WTSIdle
    pub const IDLE: i32 = 5;
    /// WTSListen
    pub const LISTEN: i32 = 6;
    /// WTSReset
    pub const RESET: i32 = 7;
    /// WTSDown
    pub const DOWN: i32 = 8;
    /// WTSInit
    pub const INIT: i32 = 9;
}

/// Seconds between 1601-01-01 (FILETIME epoch) and 1970-01-01.
const FILETIME_UNIX_OFFSET: u64 = 11_644_473_600;

/// Convert a FILETIME (100 ns ticks since 1601) to Unix seconds; zero is "unknown".
pub fn filetime_to_unix(ticks: i64) -> Option<u64> {
    let ticks = u64::try_from(ticks).ok().filter(|&ticks| ticks > 0)?;
    (ticks / 10_000_000).checked_sub(FILETIME_UNIX_OFFSET)
}

/// Values read for one session.
#[derive(Clone, Debug, Default)]
pub struct RawSession {
    /// Session id.
    pub id: u32,
    /// `pWinStationName`, such as `Console`, `RDP-Tcp#3`, `Services`.
    pub station: String,
    /// `WTS_CONNECTSTATE_CLASS`.
    pub state: i32,
    /// WTSUserName.
    pub user: String,
    /// WTSDomainName.
    pub domain: String,
    /// WTSClientName.
    pub client: String,
    /// WTSClientProtocolType: 0 console, 1 legacy ICA, 2 RDP.
    pub protocol: u16,
    /// WTSINFO LogonTime (FILETIME ticks).
    pub logon_time: i64,
    /// WTSINFO ConnectTime (FILETIME ticks).
    pub connect_time: i64,
}

/// Map raw WTS data. Listener sessions (`RDP-Tcp` with nobody signed in) are dropped.
pub fn map(raw: RawSession) -> Option<Session> {
    if raw.state == state::LISTEN || (raw.user.is_empty() && raw.id != 0) {
        return None;
    }
    let station = raw.station.to_ascii_lowercase();
    let kind = if raw.id == 0 || station == "services" {
        SessionKind::Services
    } else if raw.protocol == 2 || station.starts_with("rdp") {
        SessionKind::Rdp
    } else if station == "console" || raw.protocol == 0 {
        SessionKind::Console
    } else {
        SessionKind::Other
    };
    let state = match raw.state {
        state::ACTIVE | state::SHADOW => SessionState::Active,
        state::CONNECTED | state::CONNECT_QUERY | state::IDLE | state::INIT => SessionState::Idle,
        state::DISCONNECTED => SessionState::Disconnected,
        state::RESET | state::DOWN => SessionState::Closing,
        _ => SessionState::Other,
    };
    let text = |value: String| Some(value).filter(|value| !value.trim().is_empty());
    Some(Session {
        id: raw.id.to_string(),
        user: raw.user,
        domain: text(raw.domain),
        kind,
        state,
        client: text(raw.client).filter(|_| kind == SessionKind::Rdp),
        tty: None,
        since: filetime_to_unix(raw.logon_time).or_else(|| filetime_to_unix(raw.connect_time)),
    })
}

/// Join `NetSessionEnum` (client, user) pairs into a per-user client list.
pub fn clients_for(user: &str, sessions: &[(String, String)]) -> Option<String> {
    let mut clients: Vec<&str> = sessions
        .iter()
        .filter(|(_, owner)| owner.eq_ignore_ascii_case(user))
        .map(|(client, _)| client.trim_start_matches('\\'))
        .collect();
    clients.sort_unstable();
    clients.dedup();
    (!clients.is_empty()).then(|| clients.join(", "))
}
