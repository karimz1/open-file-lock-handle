//! The termination preview must count the windows a person sees on X11.
//!
//! A fixture child holds a file open and maps top-level windows the way real
//! apps do: several normal frames in one process, a standalone dialog, plus
//! windows that must not count (a dialog that belongs to a frame, a utility
//! palette, a dock, an override-redirect menu, an unmapped window).
//!
//! Runs only with a display: `xvfb-run` in the window impact workflow, both
//! with and without a window manager. Without `DISPLAY` the tests say so and
//! pass, because no X11 path exists to exercise.
#![cfg(target_os = "linux")]
use oflh_core::{Cancellation, Target};
use std::{
    io::{BufRead, BufReader, Write},
    path::Path,
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::Duration,
};
use x11rb::{
    connection::Connection,
    protocol::xproto::{
        AtomEnum, ConnectionExt, CreateWindowAux, EventMask, PropMode, Window, WindowClass,
    },
    wrapper::ConnectionExt as _,
};

struct Guard(Child);
impl Drop for Guard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

struct Fixture<C: Connection> {
    connection: C,
    root: Window,
}
impl<C: Connection> Fixture<C> {
    fn atom(&self, name: &str) -> u32 {
        self.connection
            .intern_atom(false, name.as_bytes())
            .unwrap()
            .reply()
            .unwrap()
            .atom
    }
    fn window(
        &self,
        title: &str,
        kind: Option<&str>,
        transient: Option<Window>,
        map: bool,
        popup: bool,
    ) -> Window {
        let window = self.connection.generate_id().unwrap();
        self.connection
            .create_window(
                x11rb::COPY_DEPTH_FROM_PARENT,
                window,
                self.root,
                40,
                40,
                320,
                200,
                0,
                WindowClass::INPUT_OUTPUT,
                x11rb::COPY_FROM_PARENT,
                &CreateWindowAux::new()
                    .override_redirect(u32::from(popup))
                    .event_mask(EventMask::STRUCTURE_NOTIFY),
            )
            .unwrap();
        let utf8 = self.atom("UTF8_STRING");
        self.connection
            .change_property8(
                PropMode::REPLACE,
                window,
                self.atom("_NET_WM_NAME"),
                utf8,
                title.as_bytes(),
            )
            .unwrap();
        self.connection
            .change_property8(
                PropMode::REPLACE,
                window,
                AtomEnum::WM_NAME,
                AtomEnum::STRING,
                title.as_bytes(),
            )
            .unwrap();
        self.connection
            .change_property32(
                PropMode::REPLACE,
                window,
                self.atom("_NET_WM_PID"),
                AtomEnum::CARDINAL,
                &[std::process::id()],
            )
            .unwrap();
        if let Some(kind) = kind {
            let kind = self.atom(kind);
            self.connection
                .change_property32(
                    PropMode::REPLACE,
                    window,
                    self.atom("_NET_WM_WINDOW_TYPE"),
                    AtomEnum::ATOM,
                    &[kind],
                )
                .unwrap();
        }
        if let Some(parent) = transient {
            self.connection
                .change_property32(
                    PropMode::REPLACE,
                    window,
                    AtomEnum::WM_TRANSIENT_FOR,
                    AtomEnum::WINDOW,
                    &[parent],
                )
                .unwrap();
        }
        if map {
            self.connection.map_window(window).unwrap();
        }
        window
    }
    /// A frame without the `_NET_WM_PID` hint, like older Xt or Motif apps.
    fn frame_without_pid_hint(&self, title: &str) -> Window {
        let window = self.frame(title);
        let hint = self.atom("_NET_WM_PID");
        self.connection.delete_property(window, hint).unwrap();
        window
    }
    fn frame(&self, title: &str) -> Window {
        self.window(title, Some("_NET_WM_WINDOW_TYPE_NORMAL"), None, true, false)
    }
}

/// Map one scenario, then wait until the parent closes stdin.
#[test]
fn x11_window_fixture() {
    let (Some(scenario), Some(hold)) = (
        std::env::var_os("OFLH_X11_SCENARIO"),
        std::env::var_os("OFLH_X11_HOLD"),
    ) else {
        return;
    };
    let _held = std::fs::File::open(hold).unwrap();
    let (connection, screen) = x11rb::connect(None).unwrap();
    let root = connection.setup().roots[screen].root;
    let f = Fixture { connection, root };
    match scenario.to_str().unwrap() {
        "single" => {
            f.frame("Fixture only.pdf");
        }
        // One process, one frame per document (one without the PID hint),
        // plus the Find dialog of the first.
        "frames" => {
            let first = f.frame("Fixture a.pdf");
            f.frame("Fixture b.pdf");
            f.frame("Fixture c.pdf");
            f.frame_without_pid_hint("Fixture d.pdf");
            f.window(
                "Fixture Find",
                Some("_NET_WM_WINDOW_TYPE_DIALOG"),
                Some(first),
                true,
                false,
            );
        }
        // Windows a person never sees as app windows must not count. An
        // untyped window without a parent and a standalone dialog do.
        "noise" => {
            f.frame("Fixture real.pdf");
            f.window("Fixture untyped.pdf", None, None, true, false);
            f.window(
                "Fixture Prompt",
                Some("_NET_WM_WINDOW_TYPE_DIALOG"),
                None,
                true,
                false,
            );
            f.window(
                "Fixture Palette",
                Some("_NET_WM_WINDOW_TYPE_UTILITY"),
                None,
                true,
                false,
            );
            f.window(
                "Fixture Panel",
                Some("_NET_WM_WINDOW_TYPE_DOCK"),
                None,
                true,
                false,
            );
            f.window("Fixture Menu", None, None, true, true);
            f.window(
                "Fixture Hidden",
                Some("_NET_WM_WINDOW_TYPE_NORMAL"),
                None,
                false,
                false,
            );
        }
        other => panic!("unknown scenario {other}"),
    }
    f.connection.flush().unwrap();
    f.connection.sync().unwrap();
    println!("X11 WINDOWS READY");
    std::io::stdout().flush().unwrap();
    let mut line = String::new();
    let _ = std::io::stdin().read_line(&mut line);
}

fn spawn(scenario: &str, hold: &Path) -> Guard {
    let mut child = Guard(
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "x11_window_fixture", "--nocapture"])
            .env("OFLH_X11_SCENARIO", scenario)
            .env("OFLH_X11_HOLD", hold)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let stdout = child.0.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            if line == "X11 WINDOWS READY" {
                let _ = tx.send(());
                break;
            }
        }
    });
    rx.recv_timeout(Duration::from_secs(15))
        .expect("x11 fixture readiness");
    // Give a window manager time to manage the new windows.
    std::thread::sleep(Duration::from_millis(1500));
    child
}

/// Scan the held file like the desktop app does, then read the preview's
/// window list for the process holding it. `None` means no display.
fn preview_windows(scenario: &str) -> Option<Vec<String>> {
    if std::env::var_os("DISPLAY").is_none() {
        println!("{scenario}: no DISPLAY; run under xvfb-run to exercise X11 windows");
        return None;
    }
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(format!("{scenario}.pdf"));
    std::fs::write(&path, b"%PDF-1.4 fixture").unwrap();
    let child = spawn(scenario, &path);
    let mut backend = oflh_platform::native().unwrap();
    let snapshot = backend
        .scan(&Target::new(&path).unwrap(), &Cancellation::default())
        .unwrap();
    let process = snapshot
        .processes
        .iter()
        .find(|process| process.identity.pid == child.0.id())
        .expect("scan must find the fixture holding the file");
    let [impact] = <[_; 1]>::try_from(backend.impact(&[process.identity])).unwrap();
    println!("{scenario}: preview windows {:?}", impact.windows);
    let mut windows = impact.windows.expect("window titles readable on X11");
    windows.sort();
    Some(windows)
}

#[test]
fn x11_preview_counts_a_single_window() {
    if let Some(windows) = preview_windows("single") {
        assert_eq!(windows, ["Fixture only.pdf"]);
    }
}

#[test]
fn x11_preview_counts_every_frame_of_one_process_but_not_its_dialog() {
    if let Some(windows) = preview_windows("frames") {
        assert_eq!(
            windows,
            [
                "Fixture a.pdf",
                "Fixture b.pdf",
                "Fixture c.pdf",
                "Fixture d.pdf"
            ]
        );
    }
}

#[test]
fn x11_preview_ignores_palettes_docks_menus_and_unmapped_windows() {
    if let Some(windows) = preview_windows("noise") {
        assert_eq!(
            windows,
            ["Fixture Prompt", "Fixture real.pdf", "Fixture untyped.pdf"]
        );
    }
}
