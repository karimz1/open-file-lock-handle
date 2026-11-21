//! Exercise the shipped non-interactive CLI with real native resources.
use std::{
    fs::{self, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::Path,
    process::{Child, Command, Output, Stdio},
    time::Duration,
};

fn check_base(command_name: &str, arguments: &[&str], path: Option<&Path>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_oflh"));
    command
        .arg(command_name)
        .args(arguments)
        .args(["--language", "en"]);
    if let Some(path) = path {
        command.arg(path);
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap()
}

fn check(arguments: &[&str], path: Option<&Path>) -> Output {
    check_base("check", arguments, path)
}
fn wait(arguments: &[&str], path: Option<&Path>) -> Output {
    check_base("wait", arguments, path)
}

fn field<'a>(json: &'a str, name: &str) -> Option<&'a str> {
    let marker = format!("\"{name}\":");
    let start = json.find(&marker)? + marker.len();
    let end = json[start..]
        .find([',', '}'])
        .map_or(json.len(), |offset| start + offset);
    Some(json[start..end].trim())
}

struct Holder(Child);
impl Holder {
    /// Close stdin so the fixture drops its handle and exits on its own, then reap it.
    fn release(mut self) {
        drop(self.0.stdin.take());
        let _ = self.0.wait();
    }
}
impl Drop for Holder {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn hold_file(path: &Path) -> Holder {
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "fixture_holder", "--nocapture"])
        .env("OFLH_CHECK_FIXTURE", path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            if line.starts_with("OFLH READY") {
                let _ = sender.send(());
                break;
            }
        }
    });
    receiver
        .recv_timeout(Duration::from_secs(15))
        .expect("holder readiness timeout");
    Holder(child)
}

#[test]
fn fixture_holder() {
    let Some(path) = std::env::var_os("OFLH_CHECK_FIXTURE") else {
        return;
    };
    let mut options = OpenOptions::new();
    options.read(true).write(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(0);
    }
    let file = options.open(path).unwrap();
    println!("OFLH READY");
    std::io::stdout().flush().unwrap();
    // Hold until the parent closes stdin (or 120 s pass), then release and exit.
    let (closed, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = std::io::copy(&mut std::io::stdin(), &mut std::io::sink());
        let _ = closed.send(());
    });
    let _ = receiver.recv_timeout(Duration::from_secs(120));
    drop(file);
}

#[test]
fn file_json_reports_a_real_holder_then_free_without_a_terminal() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("check-fixture ü.bin");
    fs::write(&path, b"fixture").unwrap();
    let holder = hold_file(&path);
    let held = check(&["--json"], Some(&path));
    assert_eq!(
        held.status.code(),
        Some(1),
        "stderr: {}",
        String::from_utf8_lossy(&held.stderr)
    );
    let stdout = String::from_utf8(held.stdout).unwrap();
    assert_eq!(stdout.lines().count(), 1, "one JSON document: {stdout}");
    assert_eq!(field(stdout.trim(), "version"), Some("1"));
    assert_eq!(field(stdout.trim(), "free"), Some("false"));
    assert!(
        stdout.contains(&format!("\"pid\":{}", holder.0.id())),
        "{stdout}"
    );
    drop(holder);
    let free = check(&["--json"], Some(&path));
    assert_eq!(
        free.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&free.stderr)
    );
    let stdout = String::from_utf8(free.stdout).unwrap();
    assert_eq!(field(stdout.trim(), "free"), Some("true"));
    assert!(stdout.contains("\"processes\":[]"), "{stdout}");
}

#[test]
fn held_file_has_a_short_human_summary() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("human-check.bin");
    fs::write(&path, b"fixture").unwrap();
    let holder = hold_file(&path);
    let output = check(&[], Some(&path));
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.starts_with("HELD:"), "{stdout}");
    assert!(
        stdout.contains(&format!("PID {}", holder.0.id())),
        "{stdout}"
    );
    drop(holder);
    let free = check(&[], Some(&path));
    assert_eq!(
        free.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&free.stderr)
    );
    assert!(String::from_utf8(free.stdout).unwrap().starts_with("FREE:"));
}

#[test]
fn port_json_reports_a_real_listener_then_free() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port().to_string();
    let held = check(&["--json", "--port", &port], None);
    assert_eq!(
        held.status.code(),
        Some(1),
        "stderr: {}",
        String::from_utf8_lossy(&held.stderr)
    );
    let stdout = String::from_utf8(held.stdout).unwrap();
    assert_eq!(field(stdout.trim(), "free"), Some("false"));
    assert!(stdout.contains(&format!("\"number\":{port}")), "{stdout}");
    drop(listener);
    let free = check(&["--json", "--port", &port], None);
    assert_eq!(
        free.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&free.stderr)
    );
    assert_eq!(
        field(String::from_utf8(free.stdout).unwrap().trim(), "free"),
        Some("true")
    );
}

#[test]
fn bad_check_arguments_use_the_usage_exit_code() {
    let output = check(&["--port", "0"], None);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(!output.stderr.is_empty());
}

#[test]
fn wait_returns_when_a_child_releases_a_real_file() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("wait-fixture.bin");
    fs::write(&path, b"fixture").unwrap();
    let holder = hold_file(&path);
    let mut waiting = Command::new(env!("CARGO_BIN_EXE_oflh"))
        .args(["wait", "--timeout", "120", "--language", "en"])
        .arg(&path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // The holder is visible to a real scan, so the wait cannot have finished yet.
    let held = check(&[], Some(&path));
    assert_eq!(held.status.code(), Some(1), "{held:?}");
    assert!(
        waiting.try_wait().unwrap().is_none(),
        "wait ended while held"
    );
    holder.release();
    let free = waiting.wait_with_output().unwrap();
    assert_eq!(
        free.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&free.stderr)
    );
    assert!(String::from_utf8(free.stdout).unwrap().starts_with("FREE:"));
}

#[test]
fn wait_timeout_names_the_remaining_holder() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("still-held.bin");
    fs::write(&path, b"fixture").unwrap();
    let holder = hold_file(&path);
    let output = wait(&["--timeout", "1"], Some(&path));
    assert_eq!(output.status.code(), Some(4));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("HELD:"), "{stderr}");
    assert!(
        stderr.contains(&format!("PID {}", holder.0.id())),
        "{stderr}"
    );
    holder.release();
}

#[test]
fn wait_timeout_and_real_port_release_have_distinct_outcomes() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port().to_string();
    let timeout = wait(&["--timeout", "1", "--port", &port], None);
    assert_eq!(
        timeout.status.code(),
        Some(4),
        "stderr: {}",
        String::from_utf8_lossy(&timeout.stderr)
    );
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(500));
        drop(listener);
    });
    let free = wait(&["--timeout", "30", "--port", &port], None);
    assert_eq!(
        free.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&free.stderr)
    );
}

#[test]
fn free_wait_succeeds_without_a_terminal() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("already-free");
    let output = wait(&[], Some(&path));
    assert_eq!(output.status.code(), Some(0));
}
