//! Real multi-user end-to-end checks, run only by the `multiuser-e2e` CI job.
//!
//! Every test is `#[ignore]` and also returns early unless
//! `OFLH_MULTIUSER_E2E=1`, so a developer machine never signs anyone out.
//! The workflow creates the users, processes, shares and sessions and passes
//! them in through environment variables:
//!
//! | Variable | Meaning |
//! | --- | --- |
//! | `OFLH_E2E_ELEVATED` | `1` when this run is admin/root, `0` for the non-admin pass |
//! | `OFLH_E2E_USER`, `OFLH_E2E_FILE`, `OFLH_E2E_PID` | a process of a second user holding a file |
//! | `OFLH_E2E_SHARE_FILE`, `OFLH_E2E_SHARE_USER` | Windows: a file opened over SMB by a third user |
//! | `OFLH_E2E_SESSION_USER`, `OFLH_E2E_SESSION_FILE`, `OFLH_E2E_SESSION_PID` | a real second login session (RDP loopback or logind) and a process in it holding a file |
//! | `OFLH_E2E_SIGN_OUT` | `1` to really sign that session out at the end |
//!
//! A test whose inputs are missing prints `SKIP <reason>` and passes, so the
//! workflow decides what is mandatory per runner (see the job's assertions).
use oflh_desktop::{
    contract::{Details, TableQuery, identity_key},
    service::Service,
};
use std::{
    path::PathBuf,
    sync::mpsc,
    time::{Duration, Instant},
};

fn enabled() -> bool {
    let on = std::env::var("OFLH_MULTIUSER_E2E").as_deref() == Ok("1");
    if !on {
        eprintln!("SKIP set OFLH_MULTIUSER_E2E=1 (CI only)");
    }
    on
}
fn var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}
fn elevated() -> bool {
    var("OFLH_E2E_ELEVATED").as_deref() == Some("1")
}
/// `CORP\max` and `max@corp` compare as `max`, case-insensitively.
fn same_user(left: &str, right: &str) -> bool {
    let bare = |user: &str| {
        let user = user.rsplit('\\').next().unwrap_or(user);
        user.split('@').next().unwrap_or(user).to_lowercase()
    };
    bare(left) == bare(right)
}
fn wait_until(what: &str, timeout: Duration, mut check: impl FnMut() -> bool) {
    let start = Instant::now();
    while !check() {
        assert!(
            start.elapsed() < timeout,
            "timed out after {timeout:?}: {what}"
        );
        std::thread::sleep(Duration::from_millis(500));
    }
}

/// Scan `target` with the real backend and return the service and revision.
fn scan(target: PathBuf) -> (Service, u32) {
    let (sender, received) = mpsc::channel();
    let service = Service::new(oflh_platform::native().unwrap(), move |status| {
        let _ = sender.send(status);
    })
    .unwrap();
    service.inspect(target).unwrap();
    loop {
        let status = received.recv_timeout(Duration::from_secs(60)).unwrap();
        assert!(status.error.is_none(), "{:?}", status.error);
        if !status.scanning && status.revision > 0 {
            return (service, status.revision);
        }
    }
}
/// Details of the scanned process `pid`, via the same calls the UI makes.
fn details_of(service: &Service, revision: u32, pid: u32) -> (String, Details) {
    let dataset = service.dataset(revision).unwrap();
    let process = dataset
        .snapshot
        .processes
        .iter()
        .find(|process| process.identity.pid == pid)
        .unwrap_or_else(|| panic!("scan did not find PID {pid}"));
    let key = identity_key(process.identity);
    let page = service
        .page(
            revision,
            &TableQuery {
                process_key: Some(key.clone()),
                limit: 50,
                ..TableQuery::default()
            },
        )
        .unwrap();
    let row = page.rows.iter().find(|row| row.pid == pid).expect("row");
    eprintln!(
        "row user={} session={:?} kind={:?}",
        row.user, row.session, row.session_kind
    );
    let details = service.details(revision, &key).unwrap();
    (key, details)
}

#[test]
#[ignore = "multi-user E2E: needs OFLH_MULTIUSER_E2E=1 and CI-created users"]
fn second_user_process_shows_owner_from_scan_to_confirmation() {
    if !enabled() {
        return;
    }
    let (Some(user), Some(file), Some(pid)) = (
        var("OFLH_E2E_USER"),
        var("OFLH_E2E_FILE"),
        var("OFLH_E2E_PID"),
    ) else {
        panic!("OFLH_E2E_USER, OFLH_E2E_FILE and OFLH_E2E_PID are required")
    };
    let pid: u32 = pid.parse().unwrap();
    let (service, revision) = scan(file.into());
    let (key, details) = details_of(&service, revision, pid);
    eprintln!(
        "owner {:?} actions {:?}",
        details.owner, details.session_actions
    );
    assert!(same_user(&details.owner.user, &user), "{:?}", details.owner);
    assert!(!details.owner.service);
    if let (Some(session), Some(actions)) = (&details.owner.session, &details.session_actions)
        && !elevated()
        && !session.own
    {
        assert!(
            matches!(actions.sign_out, "requires_admin" | "requires_root"),
            "non-elevated sign-out must be gated: {actions:?}"
        );
    }
    // The termination confirmation names the same owner. Nothing is terminated.
    let confirmation = service
        .prepare(
            revision,
            std::slice::from_ref(&key),
            false,
            oflh_platform::native().unwrap().as_mut(),
        )
        .unwrap();
    assert!(same_user(&confirmation.targets[0].owner.user, &user));
    service.dismiss();
}

#[test]
#[ignore = "multi-user E2E: needs OFLH_MULTIUSER_E2E=1"]
fn real_session_table_and_invalid_sign_out_fail_cleanly() {
    if !enabled() {
        return;
    }
    let source = oflh_sessions::native();
    let sessions = source.sessions();
    eprintln!("sessions {sessions:#?}");
    match oflh_sessions::Os::current() {
        oflh_sessions::Os::Windows => {
            let sessions = sessions.expect("WTSEnumerateSessions on the runner");
            assert!(!sessions.is_empty());
            let own = source.own_session().expect("ProcessIdToSessionId(self)");
            eprintln!("own session {own}");
        }
        oflh_sessions::Os::Linux => {
            sessions.expect("loginctl list-sessions on the runner");
        }
        _ => assert!(sessions.unwrap_or_default().is_empty()),
    }
    // A session that does not exist: an error, never a panic or a sign-out.
    let ghost = oflh_sessions::SessionKey {
        id: "987654".into(),
        user: "nobody".into(),
        since: None,
    };
    assert!(oflh_sessions::sign_out(source.as_ref(), &ghost).is_err());
    if oflh_sessions::Os::current() != oflh_sessions::Os::MacOs {
        let raw = source.sign_out("987654");
        eprintln!("raw invalid sign-out: {raw:?}");
        assert!(raw.is_err());
    }
}

#[test]
#[ignore = "multi-user E2E: Windows SMB share opened by another user"]
fn share_file_opened_by_another_user_is_listed_and_closed() {
    if !enabled() {
        return;
    }
    let (Some(file), Some(user)) = (var("OFLH_E2E_SHARE_FILE"), var("OFLH_E2E_SHARE_USER")) else {
        eprintln!("SKIP no share file (non-Windows runner)");
        return;
    };
    let (service, _) = scan(file.into());
    let shares = service.share_files();
    eprintln!("shares {shares:?}");
    if !elevated() {
        assert_eq!(shares.close, "requires_admin");
        return;
    }
    assert_eq!(shares.close, "available");
    let open = shares
        .files
        .iter()
        .find(|open| same_user(&open.user, &user))
        .unwrap_or_else(|| panic!("no share open by {user}: {shares:?}"));
    assert!(open.client.is_some(), "client computer is shown: {open:?}");
    let confirmation = service.prepare_close_file(open.id).unwrap();
    service.close_file(&confirmation.ticket).unwrap();
    assert!(
        service.close_file(&confirmation.ticket).is_err(),
        "single use"
    );
    wait_until("share open is gone", Duration::from_secs(20), || {
        service
            .share_files()
            .files
            .iter()
            .all(|other| !same_user(&other.user, &user))
    });
}

#[test]
#[ignore = "multi-user E2E: a real second login session (RDP loopback or logind)"]
fn second_session_message_and_sign_out() {
    if !enabled() {
        return;
    }
    let (Some(user), Some(file), Some(pid)) = (
        var("OFLH_E2E_SESSION_USER"),
        var("OFLH_E2E_SESSION_FILE"),
        var("OFLH_E2E_SESSION_PID"),
    ) else {
        eprintln!("SKIP no second interactive session was created on this runner");
        return;
    };
    let pid: u32 = pid.parse().unwrap();
    let source = oflh_sessions::native();
    let (service, revision) = scan(file.into());
    let (key, details) = details_of(&service, revision, pid);
    eprintln!(
        "owner {:?} actions {:?}",
        details.owner, details.session_actions
    );
    assert!(same_user(&details.owner.user, &user));
    let session = details
        .owner
        .session
        .clone()
        .expect("process has a session");
    assert!(!session.own);
    let actions = details.session_actions.expect("session actions");
    if !elevated() {
        assert!(matches!(
            actions.sign_out,
            "requires_admin" | "requires_root"
        ));
        assert!(service.prepare_sign_out(revision, &key).is_err());
        return;
    }
    if oflh_sessions::Os::current() == oflh_sessions::Os::Windows {
        assert_eq!(session.kind, "rdp");
        assert_eq!(actions.send_message, "available");
        service
            .send_message(revision, &key, "OFLH CI: this session is signed out next.")
            .unwrap();
    }
    if var("OFLH_E2E_SIGN_OUT").as_deref() != Some("1") {
        eprintln!("SKIP sign-out not requested");
        return;
    }
    assert_eq!(actions.sign_out, "available");
    let confirmation = service.prepare_sign_out(revision, &key).unwrap();
    assert!(
        confirmation
            .processes
            .items
            .iter()
            .any(|item| item.contains(&format!("PID {pid}"))),
        "{:?}",
        confirmation.processes
    );
    service.sign_out(&confirmation.ticket).unwrap();
    wait_until("session is gone", Duration::from_secs(90), || {
        source
            .sessions()
            .map(|all| {
                all.iter()
                    .all(|other| other.id != session.id || other.user.is_empty())
            })
            .unwrap_or(false)
    });
    wait_until(
        "process in the session exited",
        Duration::from_secs(60),
        || oflh_sessions::native().process_session(pid).is_none(),
    );
}

/// Non-admin pass: a standard user cannot read other users' handles, so this
/// check goes straight to the session layer for the processes the workflow
/// started and asserts every cross-user action is refused with the
/// "Requires administrator rights" / "Requires root" availability.
#[test]
#[ignore = "multi-user E2E: non-admin gating against real sessions"]
fn non_admin_cross_user_actions_are_refused() {
    if !enabled() {
        return;
    }
    if elevated() {
        eprintln!("SKIP this check is for the non-admin pass");
        return;
    }
    assert!(
        !oflh_platform::elevation::is_elevated(),
        "the non-admin pass is running elevated"
    );
    let sessions = oflh_desktop::sessions::Sessions::native();
    let targets = [
        (var("OFLH_E2E_SESSION_USER"), var("OFLH_E2E_SESSION_PID")),
        (var("OFLH_E2E_USER"), var("OFLH_E2E_PID")),
    ];
    let mut checked = 0;
    for (user, pid) in targets {
        let (Some(user), Some(pid)) = (user, pid) else {
            continue;
        };
        let pid: u32 = pid.parse().unwrap();
        let owner = sessions.owner(&user, pid);
        eprintln!("non-admin owner of {pid}: {owner:?}");
        let Some(session) = owner.session else {
            continue;
        };
        assert!(!session.own);
        let actions = sessions.actions(&session);
        assert!(
            matches!(actions.sign_out, "requires_admin" | "requires_root"),
            "{actions:?}"
        );
        if oflh_sessions::Os::current() == oflh_sessions::Os::Windows {
            assert_eq!(actions.send_message, "requires_admin");
            assert!(sessions.send_message(pid, "must be refused").is_err());
        }
        assert!(
            sessions
                .prepare_sign_out(pid, std::iter::once((pid, "locker".into())))
                .is_err()
        );
        checked += 1;
    }
    if oflh_sessions::Os::current() == oflh_sessions::Os::Windows {
        if let Some(file) = var("OFLH_E2E_SHARE_FILE") {
            let shares = sessions.share_files(Some((std::path::Path::new(&file), false)));
            eprintln!("non-admin share view: {shares:?}");
            assert_eq!(shares.close, "requires_admin");
        }
        assert!(checked > 0, "no second-user process had a readable session");
    }
}
