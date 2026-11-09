use super::*;
use std::path::Path;

fn session(id: &str, user: &str, since: u64) -> Session {
    Session {
        id: id.into(),
        user: user.into(),
        kind: SessionKind::Rdp,
        state: SessionState::Active,
        since: Some(since),
        ..Session::default()
    }
}

#[test]
fn loginctl_list_and_cgroup_parsing() {
    let list = "      3 1000 max  seat0 tty2\n     c2 1001 anna -     pts/0\n\n";
    assert_eq!(loginctl::parse_list(list), ["3", "c2"]);
    assert!(loginctl::parse_list("").is_empty());
    assert!(loginctl::parse_list("bad;id x").is_empty());
    let cgroup = "0::/user.slice/user-1001.slice/session-c2.scope\n";
    assert_eq!(loginctl::session_from_cgroup(cgroup).as_deref(), Some("c2"));
    let v1 = "12:pids:/user.slice/user-1000.slice/session-3.scope\n1:name=systemd:/user.slice/user-1000.slice/session-3.scope";
    assert_eq!(loginctl::session_from_cgroup(v1).as_deref(), Some("3"));
    assert_eq!(
        loginctl::session_from_cgroup("0::/system.slice/sshd.service"),
        None
    );
    assert_eq!(
        loginctl::session_from_cgroup("0::/user.slice/session-.scope"),
        None
    );
}

#[test]
fn loginctl_show_maps_kinds_remote_hosts_and_start_time() {
    let output = "Id=3\nName=max\nType=wayland\nClass=user\nState=active\nRemote=no\nRemoteHost=\nTTY=tty2\nService=gdm-password\nTimestampMonotonic=5000000\n\n\
Id=c2\nName=anna\nType=tty\nClass=user\nState=online\nRemote=yes\nRemoteHost=10.0.0.7\nTTY=pts/2\nService=sshd\nTimestampMonotonic=60000000\n\n\
Id=c5\nName=lee\nType=x11\nClass=user\nState=active\nRemote=yes\nRemoteHost=laptop\nService=xrdp-sesman\nTimestampMonotonic=0\n\n\
Id=7\nName=root\nType=unspecified\nClass=background\nState=closing\nRemote=no\nService=crond\n\nName=no-id\n";
    let sessions = loginctl::parse_show(output, 100.0, 1_000_100);
    assert_eq!(sessions.len(), 4);
    assert_eq!(
        (sessions[0].kind, sessions[0].state),
        (SessionKind::Wayland, SessionState::Active)
    );
    assert_eq!(sessions[0].since, Some(1_000_005));
    assert_eq!(sessions[0].client, None);
    assert_eq!(sessions[0].tty.as_deref(), Some("tty2"));
    assert_eq!(
        (sessions[1].kind, sessions[1].state),
        (SessionKind::Ssh, SessionState::Idle)
    );
    assert_eq!(sessions[1].client.as_deref(), Some("10.0.0.7"));
    assert_eq!(sessions[1].tty.as_deref(), Some("pts/2"));
    assert_eq!(sessions[2].kind, SessionKind::Rdp);
    assert_eq!(sessions[2].client.as_deref(), Some("laptop"));
    assert_eq!(sessions[2].since, None);
    assert_eq!(
        (sessions[3].kind, sessions[3].state),
        (SessionKind::Services, SessionState::Closing)
    );
    assert_eq!(loginctl::since(1, f64::NAN, 10), None);
    assert_eq!(loginctl::since(1_000_000, 20.0, 10), None);
}

#[test]
fn wts_mapping_handles_console_rdp_services_and_listeners() {
    let filetime = |unix: u64| ((unix + 11_644_473_600) * 10_000_000) as i64;
    let rdp = wts::map(wts::RawSession {
        id: 3,
        station: "RDP-Tcp#5".into(),
        state: wts::state::ACTIVE,
        user: "max.muster".into(),
        domain: "CORP".into(),
        client: "LAPTOP-7".into(),
        protocol: 2,
        console: false,
        logon_time: filetime(1_700_000_000),
        connect_time: 0,
    })
    .unwrap();
    assert_eq!(rdp.id, "3");
    assert_eq!(
        (rdp.kind, rdp.state),
        (SessionKind::Rdp, SessionState::Active)
    );
    assert_eq!(rdp.domain.as_deref(), Some("CORP"));
    assert_eq!(rdp.client.as_deref(), Some("LAPTOP-7"));
    assert_eq!(rdp.since, Some(1_700_000_000));
    let console = wts::map(wts::RawSession {
        id: 1,
        station: "Console".into(),
        state: wts::state::DISCONNECTED,
        user: "anna".into(),
        client: "ignored".into(),
        connect_time: filetime(1_600_000_000),
        ..Default::default()
    })
    .unwrap();
    assert_eq!(
        (console.kind, console.state),
        (SessionKind::Console, SessionState::Disconnected)
    );
    assert_eq!(console.client, None);
    assert_eq!(console.since, Some(1_600_000_000));
    // The active console session, whatever its station name.
    let attached = wts::map(wts::RawSession {
        id: 2,
        station: "Console".into(),
        state: wts::state::ACTIVE,
        user: "runneradmin".into(),
        console: true,
        ..Default::default()
    })
    .unwrap();
    assert_eq!(attached.kind, SessionKind::Console);
    // A disconnected RDP session loses its station name and reports protocol 0.
    // It is not the console, so it must not be labelled as one.
    let disconnected = wts::map(wts::RawSession {
        id: 1,
        station: String::new(),
        state: wts::state::DISCONNECTED,
        user: "oflh_u2".into(),
        protocol: 0,
        console: false,
        ..Default::default()
    })
    .unwrap();
    assert_eq!(
        (disconnected.kind, disconnected.state),
        (SessionKind::Other, SessionState::Disconnected)
    );
    // Protocol 2 is RDP even without a station name.
    let connected = wts::map(wts::RawSession {
        id: 5,
        state: wts::state::ACTIVE,
        user: "lee".into(),
        protocol: 2,
        ..Default::default()
    })
    .unwrap();
    assert_eq!(connected.kind, SessionKind::Rdp);
    let services = wts::map(wts::RawSession {
        id: 0,
        station: "Services".into(),
        state: wts::state::DISCONNECTED,
        ..Default::default()
    })
    .unwrap();
    assert_eq!(services.kind, SessionKind::Services);
    assert!(
        wts::map(wts::RawSession {
            id: 65536,
            station: "RDP-Tcp".into(),
            state: wts::state::LISTEN,
            ..Default::default()
        })
        .is_none()
    );
    assert!(
        wts::map(wts::RawSession {
            id: 4,
            state: wts::state::CONNECTED,
            ..Default::default()
        })
        .is_none()
    );
    assert_eq!(wts::filetime_to_unix(0), None);
    assert_eq!(wts::filetime_to_unix(-5), None);
    assert_eq!(wts::filetime_to_unix(1), None);
}

#[test]
fn share_clients_join_per_user() {
    let pairs = vec![
        ("\\\\10.0.0.5".to_string(), "max".to_string()),
        ("\\\\PC2".into(), "MAX".into()),
        ("\\\\10.0.0.5".into(), "max".into()),
        ("\\\\PC9".into(), "anna".into()),
    ];
    assert_eq!(
        wts::clients_for("max", &pairs).as_deref(),
        Some("10.0.0.5, PC2")
    );
    assert_eq!(wts::clients_for("lee", &pairs), None);
}

#[test]
fn share_file_matches_target_file_or_folder() {
    let file = ShareFile {
        id: 1,
        path: "C:\\Share\\Reports\\Q3.xlsx".into(),
        ..ShareFile::default()
    };
    assert!(file.matches(Path::new("c:/share/reports/q3.xlsx"), false));
    assert!(file.matches(Path::new("C:\\Share\\"), true));
    assert!(!file.matches(Path::new("C:\\Share"), false));
    assert!(!file.matches(Path::new("C:\\Sha"), true));
    assert!(!file.matches(Path::new(""), true));
    // Canonicalized targets carry the verbatim prefix; NetFileEnum paths do not.
    assert!(file.matches(Path::new(r"\\?\C:\Share\Reports\Q3.xlsx"), false));
    assert!(file.matches(Path::new(r"\\?\c:\share"), true));
    assert!(!file.matches(Path::new(r"\\?\C:\Sha"), true));
}

#[test]
fn availability_gates_by_platform_privilege_and_own_session() {
    use Availability::*;
    let w = Os::Windows;
    assert_eq!(
        availability(Action::SignOut, w, false, false),
        Requires(Privilege::Administrator)
    );
    assert_eq!(availability(Action::SignOut, w, false, true), Available);
    assert_eq!(availability(Action::SignOut, w, true, false), Available);
    assert_eq!(
        availability(Action::SendMessage, w, false, false),
        Requires(Privilege::Administrator)
    );
    assert_eq!(
        availability(Action::CloseShareFile, w, false, true),
        Requires(Privilege::Administrator)
    );
    assert_eq!(
        availability(Action::CloseShareFile, w, true, false),
        Available
    );
    assert_eq!(
        availability(Action::SignOut, Os::Linux, false, true),
        Requires(Privilege::Root)
    );
    assert_eq!(
        availability(Action::SignOut, Os::Linux, true, false),
        Available
    );
    assert_eq!(
        availability(Action::SendMessage, Os::Linux, true, false),
        Unsupported
    );
    assert_eq!(
        availability(Action::CloseShareFile, Os::Linux, true, false),
        Unsupported
    );
    for action in [Action::SendMessage, Action::SignOut, Action::CloseShareFile] {
        assert_eq!(availability(action, Os::MacOs, true, true), Unsupported);
    }
    assert_eq!(Requires(Privilege::Root).as_str(), "requires_root");
}

#[test]
fn actions_revalidate_the_captured_session_and_file() {
    let fake = Fake::default();
    *fake.sessions.lock().unwrap() = vec![session("3", "max", 100), session("4", "anna", 200)];
    *fake.share_files.lock().unwrap() = vec![ShareFile {
        id: 7,
        path: "C:\\S\\a.txt".into(),
        user: "anna".into(),
        ..ShareFile::default()
    }];
    let key = session("3", "max", 100).key();
    // Same id, but somebody else signed in since: refuse.
    let reused = SessionKey {
        user: "lee".into(),
        ..key.clone()
    };
    assert_eq!(sign_out(&fake, &reused), Err(Error::Changed));
    let relogon = SessionKey {
        since: Some(101),
        ..key.clone()
    };
    assert_eq!(sign_out(&fake, &relogon), Err(Error::Changed));
    assert_eq!(
        send_message(&fake, &key, "OFLH", "  "),
        Err(Error::Failed("message must be 1 to 1000 characters".into()))
    );
    send_message(&fake, &key, "OFLH", " Bitte speichern ").unwrap();
    sign_out(&fake, &key).unwrap();
    assert_eq!(sign_out(&fake, &key), Err(Error::Changed));
    let file = ShareFileKey {
        id: 7,
        path: "C:\\S\\a.txt".into(),
        user: "anna".into(),
    };
    let moved = ShareFileKey {
        path: "C:\\S\\b.txt".into(),
        ..file.clone()
    };
    assert_eq!(close_share_file(&fake, &moved), Err(Error::Changed));
    close_share_file(&fake, &file).unwrap();
    assert_eq!(
        fake.actions(),
        ["message 3 Bitte speichern", "sign_out 3", "close 7"]
    );
    let denied = Fake {
        fail: Some(Error::PermissionDenied),
        ..Fake::default()
    };
    *denied.sessions.lock().unwrap() = vec![session("3", "max", 100)];
    assert_eq!(sign_out(&denied, &key), Err(Error::PermissionDenied));
    assert!(denied.actions().is_empty());
    assert_eq!(Empty.share_files(), Err(Error::Unsupported));
    assert_eq!(Empty.sign_out("1"), Err(Error::Unsupported));
}

#[test]
fn service_accounts_are_recognised() {
    for user in [
        "SYSTEM",
        "NT AUTHORITY\\LOCAL SERVICE",
        "NetworkService",
        "root",
        "nobody",
        "NT SERVICE\\MSSQLSERVER",
        "DWM-1",
        "UMFD-2",
    ] {
        assert!(is_service_account(user), "{user}");
    }
    for user in ["max.muster", "CORP\\anna", "rooter"] {
        assert!(!is_service_account(user), "{user}");
    }
}

#[test]
fn current_os_matches_the_build() {
    let os = Os::current();
    assert_eq!(os == Os::Linux, cfg!(target_os = "linux"));
    assert_eq!(os == Os::Windows, cfg!(windows));
}
