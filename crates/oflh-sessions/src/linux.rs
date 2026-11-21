//! systemd-logind sessions through the `loginctl` CLI. Reads need no root;
//! `terminate-session` is only offered as root.
use crate::{Error, Result, Session, Source, loginctl};
use std::process::Command;

pub(crate) struct Native;

fn run(args: &[&str]) -> Result<String> {
    let output = Command::new("loginctl")
        .args(args)
        .env("LC_ALL", "C")
        .env("SYSTEMD_PAGER", "")
        .output()
        .map_err(|error| match error.kind() {
            std::io::ErrorKind::NotFound => Error::Unsupported,
            _ => Error::Failed(format!("loginctl: {error}")),
        })?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }
    let message = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    if message.contains("Access denied")
        || message.contains("not privileged")
        || message.contains("Permission denied")
    {
        Err(Error::PermissionDenied)
    } else if message.contains("No session") {
        Err(Error::Changed)
    } else {
        Err(Error::Failed(format!("loginctl: {message}")))
    }
}

fn uptime() -> f64 {
    std::fs::read_to_string("/proc/uptime")
        .ok()
        .and_then(|text| text.split_whitespace().next()?.parse().ok())
        .unwrap_or(-1.0)
}

impl Source for Native {
    fn sessions(&self) -> Result<Vec<Session>> {
        let ids = loginctl::parse_list(&run(&["list-sessions", "--no-legend", "--no-pager"])?);
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut args = vec!["show-session", "--no-pager"];
        args.extend(ids.iter().map(String::as_str));
        for property in [
            "Id",
            "Name",
            "Type",
            "Class",
            "State",
            "Remote",
            "RemoteHost",
            "TTY",
            "Service",
            "TimestampMonotonic",
        ] {
            args.extend(["-p", property]);
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|time| time.as_secs())
            .unwrap_or_default();
        Ok(loginctl::parse_show(&run(&args)?, uptime(), now))
    }
    fn process_session(&self, pid: u32) -> Option<String> {
        loginctl::session_from_cgroup(&std::fs::read_to_string(format!("/proc/{pid}/cgroup")).ok()?)
    }
    fn sign_out(&self, session: &str) -> Result<()> {
        run(&["terminate-session", "--no-pager", session]).map(drop)
    }
}
