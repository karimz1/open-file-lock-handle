//! Real child-release integration tests for the common non-destructive wait.
use oflh_core::{Cancellation, Target};
use std::{
    fs,
    path::Path,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

struct Holder(Child);
impl Drop for Holder {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn hold_file(path: &Path) -> Holder {
    let child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "wait_fixture_holder", "--nocapture"])
        .env("OFLH_WAIT_FIXTURE", path)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    let ready = path.with_extension("ready");
    while !ready.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(ready.exists(), "holder readiness timeout");
    Holder(child)
}
#[test]
fn wait_fixture_holder() {
    let Some(path) = std::env::var_os("OFLH_WAIT_FIXTURE") else {
        return;
    };
    let path = Path::new(&path);
    let mut options = fs::OpenOptions::new();
    options.read(true).write(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(0);
    }
    let _file = options.open(path).unwrap();
    fs::write(path.with_extension("ready"), b"ready").unwrap();
    std::thread::park_timeout(Duration::from_secs(120));
}
#[test]
fn child_release_completes_wait_and_timeout_does_not_report_free() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("child-release.bin");
    fs::write(&path, b"fixture").unwrap();
    let holder = hold_file(&path);
    let mut backend = oflh_platform::native().unwrap();
    let options = oflh_core::wait::Options {
        timeout: Some(Duration::from_secs(1)),
        poll_interval: Duration::from_millis(100),
    };
    let timeout = oflh_platform::wait_until_free(
        backend.as_mut(),
        oflh_platform::WaitTarget {
            path: Some(&Target::new(&path).unwrap()),
            check: oflh_core::check::CheckTarget::Path,
        },
        options,
        Instant::now(),
        &Cancellation::default(),
    );
    assert!(
        matches!(timeout, Err(oflh_core::Error::WaitTimedOut)),
        "{timeout:?}"
    );
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(250));
        drop(holder);
    });
    let options = oflh_core::wait::Options {
        timeout: Some(Duration::from_secs(30)),
        poll_interval: Duration::from_millis(100),
    };
    oflh_platform::wait_until_free(
        backend.as_mut(),
        oflh_platform::WaitTarget {
            path: Some(&Target::new(&path).unwrap()),
            check: oflh_core::check::CheckTarget::Path,
        },
        options,
        Instant::now(),
        &Cancellation::default(),
    )
    .unwrap();
}
#[test]
fn real_port_release_completes_wait() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(300));
        drop(listener);
    });
    let mut backend = oflh_platform::native().unwrap();
    let options = oflh_core::wait::Options {
        timeout: Some(Duration::from_secs(20)),
        poll_interval: Duration::from_millis(100),
    };
    oflh_platform::wait_until_free(
        backend.as_mut(),
        oflh_platform::WaitTarget {
            path: None,
            check: oflh_core::check::CheckTarget::Port(port),
        },
        options,
        Instant::now(),
        &Cancellation::default(),
    )
    .unwrap();
}
