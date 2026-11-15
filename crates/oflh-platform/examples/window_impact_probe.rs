//! Prints what the termination preview reports about the windows of every
//! process holding a path, next to a raw inventory of that process's
//! top-level windows. Used by the real-app window impact workflow.
//!
//! Usage: window_impact_probe <path> [--name <executable>]...
use oflh_core::{Cancellation, Target};
use serde_json::{Value, json};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if let Some(result) = oflh_platform::inspection_helper::dispatch() {
        return Ok(result?);
    }
    let mut arguments = std::env::args_os().skip(1);
    let target = arguments.next().ok_or("path missing")?;
    let mut names = Vec::new();
    while let Some(argument) = arguments.next() {
        if argument == "--name" {
            let name = arguments.next().ok_or("--name needs a value")?;
            names.push(name.to_string_lossy().to_lowercase());
        }
    }
    let mut backend = oflh_platform::native()?;
    let snapshot = backend.scan(&Target::new(&target)?, &Cancellation::default())?;
    let processes: Vec<_> = snapshot
        .processes
        .iter()
        .filter(|process| names.is_empty() || names.contains(&process.name.to_lowercase()))
        .collect();
    let identities: Vec<_> = processes.iter().map(|process| process.identity).collect();
    let impacts = backend.impact(&identities);
    let report: Vec<Value> = processes
        .iter()
        .zip(impacts)
        .map(|(process, impact)| {
            json!({
                "pid": process.identity.pid,
                "name": process.name,
                "windows": impact.windows,
                "inventory": inventory(process.identity.pid),
            })
        })
        .collect();
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

#[cfg(not(windows))]
fn inventory(_: u32) -> Vec<Value> {
    Vec::new()
}

#[cfg(windows)]
fn inventory(pid: u32) -> Vec<Value> {
    use windows_sys::Win32::{
        Foundation::{HWND, RECT},
        Graphics::Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute},
        UI::WindowsAndMessaging::*,
    };
    struct State {
        pid: u32,
        rows: Vec<Value>,
    }
    unsafe extern "system" fn visit(hwnd: HWND, param: isize) -> i32 {
        // SAFETY: called synchronously by EnumWindows with our live State.
        let state = unsafe { &mut *(param as *mut State) };
        let mut owner_pid = 0;
        // SAFETY: OS-supplied window; every buffer below is sized for its call.
        unsafe {
            GetWindowThreadProcessId(hwnd, &mut owner_pid);
            if owner_pid != state.pid {
                return 1;
            }
            let mut title = [0u16; 512];
            let title_length = GetWindowTextW(hwnd, title.as_mut_ptr(), title.len() as i32);
            let mut class = [0u16; 256];
            let class_length = GetClassNameW(hwnd, class.as_mut_ptr(), class.len() as i32);
            let mut rect: RECT = std::mem::zeroed();
            GetWindowRect(hwnd, &mut rect);
            let mut cloaked = 0u32;
            let cloak_read = DwmGetWindowAttribute(
                hwnd,
                DWMWA_CLOAKED as u32,
                (&mut cloaked as *mut u32).cast(),
                4,
            ) == 0;
            let owner = GetWindow(hwnd, GW_OWNER);
            state.rows.push(json!({
                "hwnd": format!("{:#x}", hwnd as usize),
                "title": String::from_utf16_lossy(&title[..title_length.max(0) as usize]),
                "class": String::from_utf16_lossy(&class[..class_length.max(0) as usize]),
                "visible": IsWindowVisible(hwnd) != 0,
                "iconic": IsIconic(hwnd) != 0,
                "rect": [rect.left, rect.top, rect.right, rect.bottom],
                "style": format!("{:#x}", GetWindowLongPtrW(hwnd, GWL_STYLE)),
                "ex_style": format!("{:#x}", GetWindowLongPtrW(hwnd, GWL_EXSTYLE)),
                "owner": (!owner.is_null()).then(|| format!("{:#x}", owner as usize)),
                "owner_visible": (!owner.is_null()).then(|| IsWindowVisible(owner) != 0),
                "cloaked": cloak_read.then_some(cloaked),
            }));
        }
        1
    }
    let mut state = State {
        pid,
        rows: Vec::new(),
    };
    // SAFETY: callback receives a pointer to state, valid for synchronous EnumWindows.
    unsafe { EnumWindows(Some(visit), (&mut state as *mut State) as isize) };
    state.rows
}
