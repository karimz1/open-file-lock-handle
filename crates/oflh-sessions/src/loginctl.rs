//! Parsing of `loginctl` and `/proc` text, kept pure for unit tests.
use crate::{Session, SessionKind, SessionState};

/// Session ids from `loginctl list-sessions --no-legend` (first column).
pub fn parse_list(output: &str) -> Vec<String> {
    output
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .filter(|id| {
            id.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        })
        .map(str::to_owned)
        .collect()
}

/// The logind session of a process, from `/proc/<pid>/cgroup`
/// (`…/session-3.scope` or `…/session-c2.scope`).
pub fn session_from_cgroup(cgroup: &str) -> Option<String> {
    cgroup.lines().find_map(|line| {
        let path = line.rsplit(':').next()?;
        path.split('/').find_map(|part| {
            part.strip_prefix("session-")?
                .strip_suffix(".scope")
                .filter(|id| !id.is_empty())
                .map(str::to_owned)
        })
    })
}

/// Time a session started, given `TimestampMonotonic` (µs since boot), the
/// current uptime (s) and the current Unix time (s).
pub fn since(monotonic_us: u64, uptime_s: f64, now_s: u64) -> Option<u64> {
    if monotonic_us == 0 || !uptime_s.is_finite() || uptime_s < 0.0 {
        return None;
    }
    let boot = now_s.checked_sub(uptime_s as u64)?;
    Some(boot + monotonic_us / 1_000_000)
}

/// Parse `loginctl show-session ID… -p Id -p Name -p Type -p Class -p State
/// -p Remote -p RemoteHost -p TTY -p Service -p TimestampMonotonic` output.
/// Blocks are separated by blank lines.
pub fn parse_show(output: &str, uptime_s: f64, now_s: u64) -> Vec<Session> {
    output
        .split("\n\n")
        .filter_map(|block| {
            let mut session = Session::default();
            let (mut kind, mut class, mut service, mut remote) =
                (String::new(), String::new(), String::new(), false);
            for line in block.lines() {
                let Some((key, value)) = line.split_once('=') else {
                    continue;
                };
                let value = value.trim();
                let text = || Some(value.to_owned()).filter(|value| !value.is_empty());
                match key.trim() {
                    "Id" => session.id = value.into(),
                    "Name" => session.user = value.into(),
                    "Type" => kind = value.into(),
                    "Class" => class = value.into(),
                    "Service" => service = value.into(),
                    "Remote" => remote = value == "yes",
                    "RemoteHost" => session.client = text(),
                    "TTY" => session.tty = text(),
                    "State" => {
                        session.state = match value {
                            "active" => SessionState::Active,
                            "online" => SessionState::Idle,
                            "closing" => SessionState::Closing,
                            _ => SessionState::Other,
                        }
                    }
                    "TimestampMonotonic" => {
                        session.since = value.parse().ok().and_then(|us| since(us, uptime_s, now_s))
                    }
                    _ => {}
                }
            }
            if session.id.is_empty() {
                return None;
            }
            session.kind = if service.contains("xrdp") {
                SessionKind::Rdp
            } else if service == "sshd" || (remote && kind == "tty") {
                SessionKind::Ssh
            } else {
                match kind.as_str() {
                    "x11" => SessionKind::X11,
                    "wayland" => SessionKind::Wayland,
                    "tty" => SessionKind::Tty,
                    _ if class == "background" || kind == "unspecified" => SessionKind::Services,
                    _ => SessionKind::Other,
                }
            };
            if !remote && session.kind != SessionKind::Rdp {
                session.client = None;
            }
            Some(session)
        })
        .collect()
}
