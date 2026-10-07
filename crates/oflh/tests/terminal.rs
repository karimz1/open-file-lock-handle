//! Exercise the shipped CLI in a real pseudo-terminal on all three platforms.
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use std::{
    io::{Read, Write},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
#[test]
fn native_terminal_workflow() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("terminal-fixture ü.bin");
    let mut file = std::fs::File::create(&path).unwrap();
    file.write_all(b"fixture").unwrap();
    let pair = native_pty_system()
        .openpty(PtySize {
            rows: 30,
            cols: 120,
            pixel_width: 0,
            pixel_height: 0,
        })
        .unwrap();
    let mut cmd = CommandBuilder::new(env!("CARGO_BIN_EXE_oflh"));
    cmd.args(["--no-update-check", "--language", "en", "--ports"]);
    cmd.arg(dir.path());
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    let mut child = pair.slave.spawn_command(cmd).unwrap();
    drop(pair.slave);
    let mut killer = child.clone_killer();
    struct Guard(Box<dyn portable_pty::ChildKiller + Send + Sync>);
    impl Drop for Guard {
        fn drop(&mut self) {
            let _ = self.0.kill();
        }
    }
    let _guard = Guard(child.clone_killer());
    let mut reader = pair.master.try_clone_reader().unwrap();
    let writer = Arc::new(Mutex::new(pair.master.take_writer().unwrap()));
    let response_writer = writer.clone();
    let parser = Arc::new(Mutex::new(vt100::Parser::new(30, 120, 0)));
    let output = parser.clone();
    std::thread::spawn(move || {
        let mut buf = [0; 8192];
        let mut terminal_requests = Vec::new();
        while let Ok(n) = reader.read(&mut buf) {
            if n == 0 {
                break;
            }
            output.lock().unwrap().process(&buf[..n]);
            terminal_requests.extend_from_slice(&buf[..n]);
            // ConPTY's INHERIT_CURSOR handshake requires the terminal host to answer DSR.
            if terminal_requests
                .windows(4)
                .any(|request| request == b"\x1b[6n")
            {
                let mut writer = response_writer.lock().unwrap();
                writer.write_all(b"\x1b[1;1R").unwrap();
                writer.flush().unwrap();
                terminal_requests.clear();
            } else if terminal_requests.len() > 16 {
                terminal_requests.drain(..terminal_requests.len() - 16);
            }
        }
    });
    let send = |input: &[u8]| {
        let mut writer = writer.lock().unwrap();
        writer.write_all(input).unwrap();
        writer.flush().unwrap();
    };
    let wait = |needle: &str| {
        let start = Instant::now();
        loop {
            let text = parser.lock().unwrap().screen().contents();
            if text.contains(needle) {
                break;
            }
            if start.elapsed() > Duration::from_secs(20) {
                panic!("did not render {needle:?}:\n{text}")
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    };
    wait("THIS PATH");
    wait(&port.to_string());
    send(b"s");
    wait("ALL PORTS");
    send(b"1");
    wait("terminal-fixture");
    send(b"/terminal-fixture\r");
    wait("1 of");
    send(b"\r");
    wait("process details");
    wait(&format!("TCP {port}"));
    send(b"l");
    wait("LOCKS ONLY");
    send(b"l");
    wait("ALL USAGES");
    send(b"r");
    std::thread::sleep(Duration::from_millis(100));
    pair.master
        .resize(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .unwrap();
    parser.lock().unwrap().screen_mut().set_size(24, 80);
    send(b"\x1b");
    wait("1 Processes");
    send(b"?");
    wait("SCAN DETAILS");
    send(b"\x1b");
    wait("1 Processes");
    send(format!("3/{port}\r").as_bytes());
    wait("ALL PORTS");
    wait(&std::process::id().to_string());
    send(b"s");
    wait("THIS PATH");
    wait(&std::process::id().to_string());
    send(b"\r");
    wait("process details · ports");
    send(b"a");
    wait("LIVE");
    send(b"\x1b");
    wait("THIS PATH");
    send(b"\r");
    wait("process details · ports");
    send(b"f");
    wait("ALL USAGES");
    send(b"q");
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        if start.elapsed() > Duration::from_secs(5) {
            let _ = killer.kill();
            panic!("quit remained blocked by scan")
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}
#[test]
fn cli_contract() {
    let bin = env!("CARGO_BIN_EXE_oflh");
    let version = std::process::Command::new(bin)
        .arg("--version")
        .output()
        .unwrap();
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).starts_with("oflh "));
    let help = std::process::Command::new(bin)
        .arg("--help")
        .output()
        .unwrap();
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("--port PORT"));
    for port in ["0", "65536", "abc"] {
        let invalid = std::process::Command::new(bin)
            .args(["--port", port])
            .output()
            .unwrap();
        assert_eq!(invalid.status.code(), Some(2));
    }
    let bad = std::process::Command::new(bin)
        .args(["one", "two"])
        .output()
        .unwrap();
    assert_eq!(bad.status.code(), Some(2));
    let piped = std::process::Command::new(bin)
        .args(["--language", "en"])
        .output()
        .unwrap();
    assert_eq!(piped.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&piped.stderr).contains("interactive terminal"));
}

#[test]
fn language_options_and_locale_precedence_work_in_the_real_cli() {
    let binary = env!("CARGO_BIN_EXE_oflh");
    for (language, heading, terminal_error) in [
        ("en", "Usage:", "interactive terminal"),
        ("de", "Aufruf:", "interaktives Terminal"),
        ("zh", "用法：", "交互式终端"),
    ] {
        let output = std::process::Command::new(binary)
            .args(["--help", "--language", language])
            .env("LC_ALL", "fr_FR.UTF-8")
            .output()
            .unwrap();
        assert!(output.status.success());
        let help = String::from_utf8(output.stdout).unwrap();
        assert!(help.contains(heading));
        assert!(help.contains("--language en|de|zh|system"));
        assert!(help.contains("--port PORT"));
        assert!(help.contains("--no-update-check"));
        let output = std::process::Command::new(binary)
            .args(["--language", language])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .contains(terminal_error)
        );
    }
    let output = std::process::Command::new(binary)
        .args(["--language", "system", "--help"])
        .env("LC_ALL", "de_DE.UTF-8")
        .env("LANG", "zh_CN.UTF-8")
        .output()
        .unwrap();
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("Aufruf:")
    );
    let output = std::process::Command::new(binary)
        .args(["--language", "invalid"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
}
