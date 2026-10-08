//! Test-only child processes exercise the desktop service against native backends.
use oflh_desktop::{
    contract::{TableQuery, identity_key},
    service::Service,
};
use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::Duration,
};
struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
/// Start this test binary as a fixture child and wait until it is ready.
fn spawn_fixture_child(hold: Option<&std::path::Path>) -> ChildGuard {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", "desktop_fixture_child", "--nocapture"])
        .env("OFLH_DESKTOP_TEST_CHILD", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped());
    if let Some(path) = hold {
        command.env("OFLH_DESKTOP_TEST_HOLD", path);
    }
    let mut child = ChildGuard(command.spawn().unwrap());
    let stdout = child.0.stdout.take().unwrap();
    let (ready_sender, ready) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            if line.unwrap() == "DESKTOP FIXTURE READY" {
                let _ = ready_sender.send(());
                break;
            }
        }
    });
    ready.recv_timeout(Duration::from_secs(10)).unwrap();
    child
}
#[test]
fn desktop_fixture_child() {
    if std::env::var_os("OFLH_DESKTOP_TEST_CHILD").is_none() {
        return;
    }
    // Hold a regular file open for the single-file target test.
    let _held =
        std::env::var_os("OFLH_DESKTOP_TEST_HOLD").map(|path| std::fs::File::open(path).unwrap());
    println!("DESKTOP FIXTURE READY");
    std::io::stdout().flush().unwrap();
    let mut line = String::new();
    std::io::stdin().read_line(&mut line).unwrap();
}
#[test]
fn native_scan_and_confirmed_force_action_use_captured_child_lifetime() {
    let executable = std::env::current_exe().unwrap();
    let mut child = spawn_fixture_child(None);
    let (sender, received) = mpsc::channel();
    let service = Service::new(oflh_platform::native().unwrap(), move |status| {
        let _ = sender.send(status);
    })
    .unwrap();
    service.inspect(executable).unwrap();
    let status = received.recv_timeout(Duration::from_secs(30)).unwrap();
    assert!(status.error.is_none(), "{:?}", status.error);
    let dataset = service.dataset(status.revision).unwrap();
    let process = dataset
        .snapshot
        .processes
        .iter()
        .find(|process| process.identity.pid == child.0.id())
        .expect("native scanner must see test child's executable");
    let key = identity_key(process.identity);
    let page = dataset
        .page(&TableQuery {
            process_key: Some(key.clone()),
            limit: 200,
            ..TableQuery::default()
        })
        .unwrap();
    assert_eq!(page.total, 1);
    assert!(dataset.path(&page.rows[0].path_ref).unwrap().is_file());
    let confirmation = service.prepare(status.revision, &[key], true).unwrap();
    let mut backend = oflh_platform::native().unwrap();
    assert!(
        backend
            .is_running(
                dataset
                    .process(&page.rows[0].process_key)
                    .unwrap()
                    .1
                    .identity
            )
            .unwrap()
    );
    let captured = dataset
        .process(&page.rows[0].process_key)
        .unwrap()
        .1
        .identity;
    assert!(
        !backend
            .is_running(oflh_core::Identity {
                started: captured.started + 1,
                ..captured
            })
            .unwrap()
    );
    let result = service
        .terminate(&confirmation.ticket, &mut *backend)
        .unwrap();
    assert!(result[0].error.is_none(), "{:?}", result[0].error);
    assert_eq!(
        result[0].outcome,
        oflh_desktop::contract::ActionOutcome::Exited
    );
    child.0.wait().unwrap();
    assert!(
        service
            .terminate(&confirmation.ticket, &mut *backend)
            .is_err()
    );
}

#[test]
fn native_port_inspection_finds_owned_tcp_and_udp_without_a_file_target() {
    let tcp = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let udp = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    let (sender, received) = mpsc::channel();
    let service = Service::new(oflh_platform::native().unwrap(), move |status| {
        let _ = sender.send(status);
    })
    .unwrap();
    service.ports().unwrap();
    let status = received.recv_timeout(Duration::from_secs(30)).unwrap();
    assert!(status.error.is_none(), "{:?}", status.error);
    assert!(status.target.is_empty());
    let dataset = service.dataset(status.revision).unwrap();
    for (number, protocol) in [
        (tcp.local_addr().unwrap().port(), "TCP"),
        (udp.local_addr().unwrap().port(), "UDP"),
    ] {
        let page = dataset
            .page(&TableQuery {
                ports: true,
                text: format!("port:{number} {protocol}"),
                limit: 200,
                ..TableQuery::default()
            })
            .unwrap();
        assert!(
            page.rows
                .iter()
                .any(|row| row.pid == std::process::id() && !row.actionable)
        );
    }
}

#[test]
fn native_single_file_target_finds_the_process_holding_it_open() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("held-report.txt");
    std::fs::write(&path, b"synthetic report").unwrap();
    // The scanners skip their own process, so a child holds the file open.
    let child = spawn_fixture_child(Some(&path));
    let (sender, received) = mpsc::channel();
    let service = Service::new(oflh_platform::native().unwrap(), move |status| {
        let _ = sender.send(status);
    })
    .unwrap();
    service.inspect(path.clone()).unwrap();
    let status = received.recv_timeout(Duration::from_secs(30)).unwrap();
    assert!(status.error.is_none(), "{:?}", status.error);
    assert!(!status.target.is_empty());
    let dataset = service.dataset(status.revision).unwrap();
    let page = dataset
        .page(&TableQuery {
            handles: true,
            limit: 200,
            ..TableQuery::default()
        })
        .unwrap();
    let canonical = std::fs::canonicalize(&path).unwrap();
    assert!(
        page.rows.iter().any(|row| {
            row.pid == child.0.id()
                && dataset.path(&row.path_ref).is_ok_and(|observed| {
                    std::fs::canonicalize(observed).ok() == Some(canonical.clone())
                })
        }),
        "a single file target must list the child process that holds it open"
    );
}

#[test]
fn native_missing_file_target_reports_a_warning_instead_of_failing() {
    let folder = tempfile::tempdir().unwrap();
    let (sender, received) = mpsc::channel();
    let service = Service::new(oflh_platform::native().unwrap(), move |status| {
        let _ = sender.send(status);
    })
    .unwrap();
    service
        .inspect(folder.path().join("missing-report.txt"))
        .unwrap();
    let status = received.recv_timeout(Duration::from_secs(30)).unwrap();
    assert!(status.error.is_none(), "{:?}", status.error);
    assert!(
        status
            .warnings
            .iter()
            .any(|warning| warning.contains("no longer exists")),
        "{:?}",
        status.warnings
    );
}
