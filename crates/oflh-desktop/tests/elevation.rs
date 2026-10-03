//! Test-only native fixture and packaged headless helper interoperability.
use oflh_core::{Cancellation, Identity, Target};
use std::{
    io::{BufRead, BufReader, Write},
    path::Path,
    process::{Child, Command, Stdio},
    time::Duration,
};
struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
#[test]
fn elevation_fixture() {
    if std::env::var_os("OFLH_ELEVATION_FIXTURE").is_none() {
        return;
    }
    println!("READY");
    std::io::stdout().flush().unwrap();
    let mut line = String::new();
    std::io::stdin().read_line(&mut line).unwrap();
}
fn request(
    executable: &Path,
    identity: Identity,
    mode: &str,
    privileged: bool,
    denied: bool,
) -> u32 {
    #[cfg(unix)]
    let mut command = {
        if privileged {
            let mut command = Command::new("/usr/bin/sudo");
            command.arg("-n");
            if denied {
                command.args(["-u", "nobody"]);
            }
            command.arg(executable);
            command
        } else {
            Command::new(executable)
        }
    };
    #[cfg(windows)]
    let mut command = {
        assert!(!denied);
        let _ = privileged;
        Command::new(executable)
    };
    let output = command
        .args([
            "--admin-terminate",
            &std::process::id().to_string(),
            &identity.pid.to_string(),
            &identity.started.to_string(),
            &identity.started_sub.to_string(),
            mode,
        ])
        .output()
        .unwrap();
    #[cfg(unix)]
    {
        assert!(
            output.status.success(),
            "helper launcher failed (privileged={privileged}, denied={denied}, mode={mode}): {:?}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout)
            .unwrap()
            .trim()
            .parse()
            .unwrap()
    }
    #[cfg(windows)]
    {
        output.status.code().unwrap() as u32
    }
}
#[test]
#[ignore = "requires the built desktop executable and native administrator privileges; run by the six-target desktop CI matrix"]
fn packaged_helper_validates_identity_permissions_and_force_action() {
    let executable = std::env::var_os("OFLH_ADMIN_HELPER_EXE")
        .expect("CI must supply the exact built executable");
    let executable = Path::new(&executable);
    #[cfg(windows)]
    assert!(
        oflh_platform::elevation::is_elevated(),
        "Windows native CI runner must be elevated"
    );
    exercise_helper(executable, true);
}
#[cfg(feature = "desktop")]
#[test]
fn headless_helper_runs_without_a_webview_and_retains_native_guards() {
    exercise_helper(Path::new(env!("CARGO_BIN_EXE_oflh-desktop")), false);
}
fn exercise_helper(executable: &Path, privileged: bool) {
    let test_executable = std::env::current_exe().unwrap();
    let mut child = ChildGuard(
        Command::new(&test_executable)
            .args(["--exact", "elevation_fixture", "--nocapture"])
            .env("OFLH_ELEVATION_FIXTURE", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let stdout = child.0.stdout.take().unwrap();
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            if line.unwrap() == "READY" {
                let _ = sender.send(());
                break;
            }
        }
    });
    receiver.recv_timeout(Duration::from_secs(10)).unwrap();
    let mut backend = oflh_platform::native().unwrap();
    let snapshot = backend
        .scan(
            &Target::new(test_executable).unwrap(),
            &Cancellation::default(),
        )
        .unwrap();
    let identity = snapshot
        .processes
        .iter()
        .find(|process| process.identity.pid == child.0.id())
        .expect("fixture must be observed")
        .identity;
    assert_eq!(
        request(
            executable,
            Identity {
                started: identity.started + 1,
                ..identity
            },
            "1",
            privileged,
            false
        ),
        1
    );
    assert!(backend.is_running(identity).unwrap());
    assert_eq!(
        request(
            executable,
            Identity { pid: 1, ..identity },
            "1",
            privileged,
            false
        ),
        2
    );
    assert_eq!(
        request(
            executable,
            Identity {
                pid: std::process::id(),
                ..identity
            },
            "1",
            privileged,
            false
        ),
        2
    );
    assert_eq!(request(executable, identity, "2", privileged, false), 4);
    #[cfg(unix)]
    if privileged {
        use std::os::unix::fs::PermissionsExt;
        // Hosted runner home directories may be inaccessible to nobody. Copy the
        // exact tested helper bytes into a traversable test-only directory.
        let directory = tempfile::Builder::new()
            .prefix("oflh-helper-test-")
            .tempdir_in("/tmp")
            .unwrap();
        std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
        let denied_executable = directory.path().join("oflh-helper");
        std::fs::copy(executable, &denied_executable).unwrap();
        std::fs::set_permissions(&denied_executable, std::fs::Permissions::from_mode(0o755))
            .unwrap();
        let denied = request(&denied_executable, identity, "1", privileged, true);
        assert!(
            denied >= 256,
            "different-user request must retain a native error"
        );
        assert_eq!(
            std::io::Error::from_raw_os_error((denied - 256) as i32).kind(),
            std::io::ErrorKind::PermissionDenied
        );
        assert!(backend.is_running(identity).unwrap());
    }
    assert_eq!(request(executable, identity, "1", privileged, false), 0);
    child.0.wait().unwrap();
    assert!(!backend.is_running(identity).unwrap());
}
