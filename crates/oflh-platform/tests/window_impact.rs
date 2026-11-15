//! The termination preview must count the windows a person actually sees.
//!
//! A fixture child holds a file open and builds top-level windows the way real
//! multi-window apps do: frames owned by a hidden host window, owned frames
//! with their own taskbar button, a VCL-style zero-size host, plus windows that
//! must not count (owned dialogs, tool palettes, zero-size, hidden, cloaked).
#![cfg(windows)]
use oflh_core::{Cancellation, Target};
use std::{
    io::{BufRead, BufReader, Write},
    path::Path,
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::Duration,
};
use windows_sys::Win32::{
    Foundation::HWND,
    Graphics::Dwm::{DWMWA_CLOAK, DwmSetWindowAttribute},
    System::LibraryLoader::GetModuleHandleW,
    UI::WindowsAndMessaging::*,
};

struct Guard(Child);
impl Drop for Guard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}

struct Builder {
    class: Vec<u16>,
    cloak_supported: bool,
}
impl Builder {
    fn new() -> Self {
        let class = wide("OflhWindowImpactFixture");
        // SAFETY: the class name buffer outlives registration; DefWindowProcW has the WNDPROC signature.
        unsafe {
            let window_class = WNDCLASSW {
                lpfnWndProc: Some(DefWindowProcW),
                hInstance: GetModuleHandleW(std::ptr::null()),
                lpszClassName: class.as_ptr(),
                ..std::mem::zeroed()
            };
            assert_ne!(RegisterClassW(&window_class), 0, "register fixture class");
        }
        Self {
            class,
            cloak_supported: true,
        }
    }
    fn window(&self, title: &str, ex: u32, style: u32, owner: HWND, size: i32) -> HWND {
        let title = wide(title);
        // SAFETY: registered class, valid title buffer; owner is null or a window of this thread.
        let hwnd = unsafe {
            CreateWindowExW(
                ex,
                self.class.as_ptr(),
                title.as_ptr(),
                style,
                40,
                40,
                size,
                size,
                owner,
                std::ptr::null_mut(),
                GetModuleHandleW(std::ptr::null()),
                std::ptr::null(),
            )
        };
        assert!(!hwnd.is_null(), "create fixture window");
        hwnd
    }
    fn frame(&self, title: &str, owner: HWND) -> HWND {
        self.window(title, 0, WS_OVERLAPPEDWINDOW | WS_VISIBLE, owner, 320)
    }
    fn hidden(&self, title: &str) -> HWND {
        self.window(title, 0, WS_OVERLAPPEDWINDOW, std::ptr::null_mut(), 320)
    }
    fn cloaked(&mut self, title: &str) {
        let hwnd = self.window(title, 0, WS_OVERLAPPEDWINDOW, std::ptr::null_mut(), 320);
        let on: i32 = 1;
        // SAFETY: own window, BOOL-sized input buffer.
        let result = unsafe {
            DwmSetWindowAttribute(hwnd, DWMWA_CLOAK as u32, (&on as *const i32).cast(), 4)
        };
        if result != 0 {
            self.cloak_supported = false;
            return;
        }
        // SAFETY: own window; cloaked first so it never paints.
        unsafe { ShowWindow(hwnd, SW_SHOWNOACTIVATE) };
    }
}

/// Build one scenario in this process and keep pumping messages until the
/// parent closes stdin.
#[test]
fn window_impact_fixture() {
    let (Some(scenario), Some(hold)) = (
        std::env::var_os("OFLH_WINDOW_SCENARIO"),
        std::env::var_os("OFLH_WINDOW_HOLD"),
    ) else {
        return;
    };
    let _held = std::fs::File::open(hold).unwrap();
    let mut b = Builder::new();
    let none = std::ptr::null_mut();
    match scenario.to_str().unwrap() {
        "single" => {
            b.frame("Fixture only.pdf", none);
        }
        // MFC/Win32 multi-frame apps: one hidden host owns every document frame.
        "hidden-owner" => {
            let host = b.hidden("Fixture Host");
            for name in ["a.pdf", "b.pdf", "c.pdf"] {
                b.frame(&format!("Fixture {name}"), host);
            }
        }
        // A visible main frame owns a second frame that asks for its own taskbar
        // button, and a dialog that does not.
        "app-window" => {
            let main = b.frame("Fixture main.pdf", none);
            b.window(
                "Fixture second.pdf",
                WS_EX_APPWINDOW,
                WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                main,
                320,
            );
            b.window(
                "Fixture Find",
                WS_EX_DLGMODALFRAME,
                WS_POPUP | WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
                main,
                200,
            );
        }
        // Classic VCL: a visible zero-size application window owns the forms.
        "zero-size-host" => {
            let host = b.window("Fixture Delphi Host", 0, WS_POPUP | WS_VISIBLE, none, 0);
            b.frame("Fixture form.pdf", host);
        }
        // Windows a person never sees must not inflate the count.
        "noise" => {
            b.frame("Fixture real.pdf", none);
            b.window(
                "Fixture minimized.pdf",
                0,
                WS_OVERLAPPEDWINDOW | WS_VISIBLE | WS_MINIMIZE,
                none,
                320,
            );
            b.window(
                "Fixture Palette",
                WS_EX_TOOLWINDOW,
                WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                none,
                200,
            );
            b.window("Fixture Zero", 0, WS_POPUP | WS_VISIBLE, none, 0);
            b.hidden("Fixture Invisible");
            b.cloaked("Fixture Cloaked");
        }
        other => panic!("unknown scenario {other}"),
    }
    println!("WINDOWS READY cloak={}", b.cloak_supported);
    std::io::stdout().flush().unwrap();
    std::thread::spawn(|| {
        let mut line = String::new();
        let _ = std::io::stdin().read_line(&mut line);
        std::process::exit(0);
    });
    // SAFETY: standard message loop on the thread that owns the windows.
    unsafe {
        let mut message = std::mem::zeroed();
        while GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

fn spawn(scenario: &str, hold: &Path) -> (Guard, bool) {
    let mut child = Guard(
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "window_impact_fixture", "--nocapture"])
            .env("OFLH_WINDOW_SCENARIO", scenario)
            .env("OFLH_WINDOW_HOLD", hold)
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
            if let Some(cloak) = line.strip_prefix("WINDOWS READY cloak=") {
                let _ = tx.send(cloak == "true");
                break;
            }
        }
    });
    let cloak = rx
        .recv_timeout(Duration::from_secs(15))
        .expect("window fixture readiness");
    (child, cloak)
}

fn native() -> Box<dyn oflh_platform::Backend> {
    oflh_platform::native_with_inspection_helper(
        oflh_platform::inspection_helper::InspectionHelperCommand::new(
            std::env::current_exe().unwrap(),
            ["--exact", "window_impact_helper", "--nocapture"]
                .map(std::ffi::OsString::from)
                .to_vec(),
        ),
    )
    .unwrap()
}
#[test]
fn window_impact_helper() {
    if std::env::var_os("OFLH_NATIVE_HANDLE_HELPER").is_none() {
        return;
    }
    let result = oflh_platform::inspection_helper::run_stdio();
    std::process::exit(if result.is_ok() { 0 } else { 1 });
}

/// Scan the held file like the desktop app does, then read the preview's
/// window list for the process that holds it.
fn preview_windows(scenario: &str) -> (Vec<String>, bool) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(format!("{scenario}.pdf"));
    std::fs::write(&path, b"%PDF-1.4 fixture").unwrap();
    let (child, cloak) = spawn(scenario, &path);
    let mut backend = native();
    let snapshot = backend
        .scan(&Target::new(&path).unwrap(), &Cancellation::default())
        .unwrap();
    let process = snapshot
        .processes
        .iter()
        .find(|process| process.identity.pid == child.0.id())
        .expect("scan must find the fixture holding the file");
    let [impact] = <[_; 1]>::try_from(backend.impact(&[process.identity])).unwrap();
    let mut windows = impact.windows.expect("window titles readable");
    windows.sort();
    println!("{scenario}: preview windows {windows:?}");
    (windows, cloak)
}

#[test]
fn preview_counts_a_single_window() {
    assert_eq!(preview_windows("single").0, ["Fixture only.pdf"]);
}

#[test]
fn preview_counts_frames_owned_by_a_hidden_host_window() {
    assert_eq!(
        preview_windows("hidden-owner").0,
        ["Fixture a.pdf", "Fixture b.pdf", "Fixture c.pdf"],
        "every visible document frame closes with the process"
    );
}

#[test]
fn preview_counts_owned_frames_with_their_own_taskbar_button_but_not_dialogs() {
    assert_eq!(
        preview_windows("app-window").0,
        ["Fixture main.pdf", "Fixture second.pdf"]
    );
}

#[test]
fn preview_counts_forms_of_a_zero_size_application_window() {
    assert_eq!(preview_windows("zero-size-host").0, ["Fixture form.pdf"]);
}

#[test]
fn preview_ignores_tool_zero_size_hidden_and_cloaked_windows() {
    let (windows, cloak) = preview_windows("noise");
    if !cloak {
        println!("DWM cloaking unavailable in this session; the cloaked case is not exercised");
    }
    assert_eq!(windows, ["Fixture minimized.pdf", "Fixture real.pdf"]);
}
