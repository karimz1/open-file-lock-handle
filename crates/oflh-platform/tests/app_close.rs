//! Closing a multi-process app through OFLH must free the file, not just end
//! the helper that held it.
//!
//! Browser-engine apps (and many others) keep files open in helper processes
//! that run the same executable as the main process. When a helper ends, the
//! main process starts a new one, which opens the file again. The fixture
//! below does exactly that, with this test binary as both the app and its
//! helper, and shows a real top-level window so it counts as a visible app:
//! a Win32 window on Windows, an X11 window on Linux (on the display in
//! `DISPLAY`, or on a private Xvfb started here), and the main executable of
//! a copied `.app` bundle on macOS. No third-party app is involved.
//!
//! The harness is custom (`harness = false`) so this binary can also be the
//! fixture, and so Linux can start Xvfb before any thread exists.
use oflh_core::{Cancellation, Identity, Target};
use oflh_platform::Backend;
use std::{
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};

const ROLE: &str = "OFLH_APP_ROLE";
const HOLD: &str = "OFLH_APP_HOLD";
const HELPER_EXE: &str = "OFLH_APP_HELPER_EXE";
/// Helpers that keep running when the app is gone, so only OFLH can end them.
const STAYS: &str = "OFLH_APP_HELPER_STAYS";

fn main() {
    match std::env::var(ROLE).as_deref() {
        Ok("helper") => return helper(),
        Ok("app") => return app(),
        _ => {}
    }
    #[cfg(target_os = "linux")]
    let _display = display::ensure();
    let filter: Vec<String> = std::env::args()
        .skip(1)
        .filter(|argument| !argument.starts_with('-'))
        .collect();
    let tests: [(&str, fn()); 5] = [
        (
            "the_helper_resolves_to_its_visible_app",
            the_helper_resolves_to_its_visible_app,
        ),
        (
            "closing_only_the_helper_leaves_the_file_held",
            closing_only_the_helper_leaves_the_file_held,
        ),
        (
            "normal_close_of_the_app_frees_the_file",
            normal_close_of_the_app_frees_the_file,
        ),
        (
            "forced_close_ends_the_app_and_its_helpers",
            forced_close_ends_the_app_and_its_helpers,
        ),
        (
            "a_parent_running_another_program_is_never_closed",
            a_parent_running_another_program_is_never_closed,
        ),
    ];
    let mut failed = Vec::new();
    for (name, test) in tests {
        if !filter.is_empty() && !filter.iter().any(|wanted| name.contains(wanted.as_str())) {
            continue;
        }
        println!("test {name} ...");
        let started = Instant::now();
        match std::panic::catch_unwind(test) {
            Ok(()) => println!("test {name} ... ok ({:?})", started.elapsed()),
            Err(_) => {
                println!("test {name} ... FAILED ({:?})", started.elapsed());
                failed.push(name)
            }
        }
    }
    if failed.is_empty() {
        println!("test result: ok");
    } else {
        println!("test result: FAILED: {failed:?}");
        std::process::exit(101);
    }
}

// ---------------------------------------------------------------- fixture

/// Hold the file, report readiness, then wait for the app (stdin) or forever.
fn helper() {
    let path = std::env::var_os(HOLD).unwrap();
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .unwrap();
    println!("HELPER READY {}", std::process::id());
    std::io::stdout().flush().unwrap();
    if std::env::var_os(STAYS).is_some() {
        loop {
            std::thread::sleep(Duration::from_secs(60));
        }
    }
    let mut line = String::new();
    // Returns when the app ends and the pipe closes.
    let _ = std::io::stdin().read_line(&mut line);
    drop(file);
}

fn spawn_helper() -> Child {
    let executable = std::env::var_os(HELPER_EXE)
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_exe().unwrap());
    let stays = std::env::var_os(STAYS).is_some();
    let mut helper = Command::new(executable)
        .env(ROLE, "helper")
        .stdin(if stays { Stdio::null() } else { Stdio::piped() })
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    ready(&mut helper, "HELPER READY ");
    helper
}

/// A visible app that keeps one helper alive, like a browser engine does
/// with its network or utility process.
fn app() {
    let window = visible_window();
    let mut helper = spawn_helper();
    println!("APP READY {}", helper.id());
    std::io::stdout().flush().unwrap();
    std::thread::spawn(move || {
        loop {
            if helper.try_wait().unwrap().is_some() {
                helper = spawn_helper();
                println!("RESPAWNED {}", helper.id());
                std::io::stdout().flush().unwrap();
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    });
    run_until_closed(window);
    std::process::exit(0);
}

#[cfg(windows)]
mod win {
    use windows_sys::Win32::{
        Foundation::{HWND, LPARAM, LRESULT, WPARAM},
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::*,
    };
    unsafe extern "system" fn procedure(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        // SAFETY: forwarded from the OS with its own arguments.
        unsafe {
            if message == WM_DESTROY {
                PostQuitMessage(0);
                return 0;
            }
            DefWindowProcW(hwnd, message, wparam, lparam)
        }
    }
    pub fn window() -> HWND {
        let class: Vec<u16> = "OflhAppCloseFixture\0".encode_utf16().collect();
        let title: Vec<u16> = "OFLH app close fixture\0".encode_utf16().collect();
        // SAFETY: buffers outlive the calls; the procedure has the WNDPROC signature.
        unsafe {
            let window_class = WNDCLASSW {
                lpfnWndProc: Some(procedure),
                hInstance: GetModuleHandleW(std::ptr::null()),
                lpszClassName: class.as_ptr(),
                ..std::mem::zeroed()
            };
            assert_ne!(RegisterClassW(&window_class), 0);
            let hwnd = CreateWindowExW(
                0,
                class.as_ptr(),
                title.as_ptr(),
                WS_OVERLAPPEDWINDOW,
                40,
                40,
                320,
                200,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                GetModuleHandleW(std::ptr::null()),
                std::ptr::null(),
            );
            assert!(!hwnd.is_null());
            ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            hwnd
        }
    }
    pub fn pump() {
        // SAFETY: standard message loop on the thread that owns the window.
        unsafe {
            let mut message = std::mem::zeroed();
            while GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }
}
#[cfg(windows)]
fn visible_window() -> windows_sys::Win32::Foundation::HWND {
    win::window()
}
#[cfg(windows)]
fn run_until_closed(_: windows_sys::Win32::Foundation::HWND) {
    // WM_CLOSE destroys the window, WM_DESTROY ends the loop: the app quits.
    win::pump()
}

#[cfg(target_os = "linux")]
fn visible_window() -> impl Sized {
    use x11rb::{
        connection::Connection,
        protocol::xproto::{AtomEnum, ConnectionExt, CreateWindowAux, PropMode, WindowClass},
        wrapper::ConnectionExt as _,
    };
    let (connection, screen) = x11rb::connect(None).unwrap();
    let root = connection.setup().roots[screen].root;
    let window = connection.generate_id().unwrap();
    connection
        .create_window(
            x11rb::COPY_DEPTH_FROM_PARENT,
            window,
            root,
            40,
            40,
            320,
            200,
            0,
            WindowClass::INPUT_OUTPUT,
            x11rb::COPY_FROM_PARENT,
            &CreateWindowAux::new(),
        )
        .unwrap();
    let atom = |name: &str| {
        connection
            .intern_atom(false, name.as_bytes())
            .unwrap()
            .reply()
            .unwrap()
            .atom
    };
    let title = b"OFLH app close fixture";
    connection
        .change_property8(
            PropMode::REPLACE,
            window,
            atom("_NET_WM_NAME"),
            atom("UTF8_STRING"),
            title,
        )
        .unwrap();
    connection
        .change_property8(
            PropMode::REPLACE,
            window,
            AtomEnum::WM_NAME,
            AtomEnum::STRING,
            title,
        )
        .unwrap();
    connection
        .change_property32(
            PropMode::REPLACE,
            window,
            atom("_NET_WM_PID"),
            AtomEnum::CARDINAL,
            &[std::process::id()],
        )
        .unwrap();
    connection.map_window(window).unwrap();
    connection.sync().unwrap();
    connection
}
/// macOS shows the main executable of a bundle as the app; the harness
/// starts this process from `Fixture.app/Contents/MacOS/Fixture`.
#[cfg(target_os = "macos")]
struct BundleApp;
#[cfg(target_os = "macos")]
fn visible_window() -> BundleApp {
    BundleApp
}
#[cfg(unix)]
fn run_until_closed<T>(_window: T) {
    // SIGTERM (a normal close on Linux and macOS) ends the process; the
    // helper sees its pipe close and exits, like a real app's helpers.
    loop {
        std::thread::sleep(Duration::from_secs(60));
    }
}

#[cfg(target_os = "linux")]
mod display {
    use std::io::{BufRead, BufReader};
    use std::process::{Child, Command, Stdio};
    pub struct Display(Option<Child>);
    impl Drop for Display {
        fn drop(&mut self) {
            if let Some(child) = &mut self.0 {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
    /// Use the current X display, or start a private Xvfb. Runs before any
    /// thread exists, so setting `DISPLAY` is race-free.
    pub fn ensure() -> Display {
        if std::env::var_os("DISPLAY").is_some() {
            return Display(None);
        }
        let mut xvfb = Command::new("Xvfb")
            .args([
                "-displayfd",
                "1",
                "-nolisten",
                "tcp",
                "-screen",
                "0",
                "800x600x24",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("Linux app-close tests need an X display: set DISPLAY or install Xvfb");
        let mut number = String::new();
        BufReader::new(xvfb.stdout.take().unwrap())
            .read_line(&mut number)
            .unwrap();
        let number = number.trim();
        assert!(!number.is_empty(), "Xvfb did not report a display");
        // SAFETY: single-threaded at this point (custom harness, first call in main).
        unsafe { std::env::set_var("DISPLAY", format!(":{number}")) };
        Display(Some(xvfb))
    }
}

// ---------------------------------------------------------------- harness

fn ready(child: &mut Child, prefix: &'static str) -> u32 {
    let stdout = child.stdout.take().unwrap();
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            if let Some(pid) = line.strip_prefix(prefix) {
                let _ = sender.send(pid.trim().parse::<u32>().unwrap());
            }
        }
    });
    receiver
        .recv_timeout(Duration::from_secs(20))
        .unwrap_or_else(|_| panic!("fixture never printed {prefix}"))
}

/// One running fixture app, its file, and cleanup of anything left behind.
struct Fixture {
    app: Child,
    helper: u32,
    file: PathBuf,
    _directory: tempfile::TempDir,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.app.kill();
        let _ = self.app.wait();
        // Helpers that outlive the app would keep the temp file open.
        if let Ok(mut backend) = oflh_platform::native() {
            for holder in holders(&mut *backend, &self.file) {
                let _ = backend.terminate(holder, true, &Cancellation::default());
            }
        }
    }
}

/// App and helper executables: this binary, or on macOS a copied bundle with
/// the helper nested inside, the layout browser-engine apps use.
fn executables(directory: &Path) -> (PathBuf, PathBuf) {
    let current = std::env::current_exe().unwrap();
    if !cfg!(target_os = "macos") {
        return (current.clone(), current);
    }
    let bundle = directory.join("Fixture.app/Contents");
    let app = bundle.join("MacOS/Fixture");
    let helper = bundle.join("Frameworks/Fixture Helper.app/Contents/MacOS/Fixture Helper");
    for path in [&app, &helper] {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::copy(&current, path).unwrap();
    }
    (app, helper)
}

fn start(stays: bool) -> Fixture {
    let directory = tempfile::tempdir().unwrap();
    let file = std::fs::canonicalize(directory.path())
        .unwrap()
        .join("held by a helper.bin");
    std::fs::write(&file, vec![0; 4096]).unwrap();
    let (app, helper) = executables(directory.path());
    let mut command = Command::new(app);
    command
        .env(ROLE, "app")
        .env(HOLD, &file)
        .env(HELPER_EXE, helper)
        .stdin(Stdio::null())
        .stdout(Stdio::piped());
    if stays {
        command.env(STAYS, "1");
    }
    let mut app = command.spawn().unwrap();
    let helper = ready(&mut app, "APP READY ");
    Fixture {
        app,
        helper,
        file,
        _directory: directory,
    }
}

fn holders(backend: &mut dyn Backend, file: &Path) -> Vec<Identity> {
    backend
        .scan(&Target::new(file).unwrap(), &Cancellation::default())
        .map(|snapshot| {
            snapshot
                .processes
                .into_iter()
                .filter(|process| process.identity.pid != 0 && !process.usages.is_empty())
                .map(|process| process.identity)
                .collect()
        })
        .unwrap_or_default()
}

fn holder(backend: &mut dyn Backend, fixture: &Fixture) -> Identity {
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(found) = holders(backend, &fixture.file)
            .into_iter()
            .find(|identity| identity.pid == fixture.helper)
        {
            return found;
        }
        assert!(
            Instant::now() < until,
            "helper {} not found",
            fixture.helper
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// The app OFLH resolves for the holder, as the confirmation reads it. A
/// window can take a moment to be listed, so this retries briefly.
fn resolved_app(backend: &mut dyn Backend, holder: Identity) -> Option<oflh_core::app::AppRoot> {
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        let [impact] = <[_; 1]>::try_from(backend.impact(&[holder])).unwrap();
        if impact.app.is_some() || Instant::now() >= until {
            return impact.app;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn wait_until_gone(backend: &mut dyn Backend, identity: Identity) {
    let until = Instant::now() + Duration::from_secs(10);
    while backend.is_running(identity).unwrap() {
        assert!(Instant::now() < until, "PID {} still running", identity.pid);
        std::thread::sleep(Duration::from_millis(50));
    }
}

// ---------------------------------------------------------------- tests

fn the_helper_resolves_to_its_visible_app() {
    let fixture = start(false);
    let mut backend = oflh_platform::native().unwrap();
    let holder = holder(&mut *backend, &fixture);
    let app = resolved_app(&mut *backend, holder).expect("the helper's app must be resolved");
    assert_eq!(app.identity.pid, fixture.app.id());
    assert!(backend.is_running(app.identity).unwrap());
}

/// Why a single-process close is not enough: the app starts a new helper,
/// which holds the file again.
fn closing_only_the_helper_leaves_the_file_held() {
    let fixture = start(false);
    let mut backend = oflh_platform::native().unwrap();
    let holder = holder(&mut *backend, &fixture);
    backend
        .terminate(holder, true, &Cancellation::default())
        .unwrap();
    wait_until_gone(&mut *backend, holder);
    let again = oflh_platform::holders_after(
        &mut *backend,
        std::slice::from_ref(&fixture.file),
        &[holder],
        &Cancellation::default(),
    )
    .expect("the file can be checked");
    assert!(
        !again.is_empty(),
        "the app should have started a new helper that holds the file again"
    );
    assert!(again.iter().all(|process| process.identity != holder));
}

fn normal_close_of_the_app_frees_the_file() {
    let fixture = start(false);
    let mut backend = oflh_platform::native().unwrap();
    let holder = holder(&mut *backend, &fixture);
    let app = resolved_app(&mut *backend, holder).expect("app resolved");
    backend
        .terminate_app(app.identity, false, &Cancellation::default())
        .unwrap();
    wait_until_gone(&mut *backend, app.identity);
    assert_eq!(
        oflh_platform::holders_after(
            &mut *backend,
            std::slice::from_ref(&fixture.file),
            &[app.identity, holder],
            &Cancellation::default(),
        ),
        Some(Vec::new()),
        "closing the app must free the file"
    );
}

fn forced_close_ends_the_app_and_its_helpers() {
    // These helpers do not end with the app, so OFLH has to end them.
    let fixture = start(true);
    let mut backend = oflh_platform::native().unwrap();
    let holder = holder(&mut *backend, &fixture);
    let app = resolved_app(&mut *backend, holder).expect("app resolved");
    assert_eq!(backend.app_helpers(app.identity), vec![holder]);
    backend
        .terminate_app(app.identity, true, &Cancellation::default())
        .unwrap();
    wait_until_gone(&mut *backend, app.identity);
    wait_until_gone(&mut *backend, holder);
    assert_eq!(
        oflh_platform::holders_after(
            &mut *backend,
            std::slice::from_ref(&fixture.file),
            &[app.identity, holder],
            &Cancellation::default(),
        ),
        Some(Vec::new()),
        "a forced close must end the app and every helper it left behind"
    );
}

/// A helper started through a shell (another program) has no app above it:
/// OFLH must close only the helper and never the shell or what started it.
fn a_parent_running_another_program_is_never_closed() {
    let directory = tempfile::tempdir().unwrap();
    let file = std::fs::canonicalize(directory.path())
        .unwrap()
        .join("held under a shell.bin");
    std::fs::write(&file, vec![0; 4096]).unwrap();
    let current = std::env::current_exe().unwrap();
    #[cfg(windows)]
    let mut shell = {
        let mut command = Command::new("cmd.exe");
        command.arg("/d").arg("/c").arg(&current);
        command
    };
    #[cfg(unix)]
    let mut shell = {
        let mut command = Command::new("/bin/sh");
        // Not the last command, so the shell cannot exec into the helper.
        command
            .arg("-c")
            .arg("\"$0\"; status=$?; exit $status")
            .arg(&current);
        command
    };
    let mut shell = shell
        .env(ROLE, "helper")
        .env(HOLD, &file)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let helper_pid = ready(&mut shell, "HELPER READY ");
    let mut backend = oflh_platform::native().unwrap();
    let until = Instant::now() + Duration::from_secs(10);
    let holder = loop {
        if let Some(found) = holders(&mut *backend, &file)
            .into_iter()
            .find(|identity| identity.pid == helper_pid)
        {
            break found;
        }
        assert!(Instant::now() < until, "helper not found");
        std::thread::sleep(Duration::from_millis(100));
    };
    let [impact] = <[_; 1]>::try_from(backend.impact(&[holder])).unwrap();
    assert_eq!(impact.app, None);
    drop(shell.stdin.take());
    let _ = shell.wait();
}
