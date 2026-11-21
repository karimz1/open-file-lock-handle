//! Reveal a path in Explorer and select it, including when this process is elevated.
use super::*;
use std::{thread, time::Duration};
use windows_sys::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};
use windows_sys::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows_sys::Win32::UI::Shell::{ILCreateFromPathW, ILFree, SHOpenFolderAndSelectItems};

/// Open Explorer and select `path`.
///
/// When this process is elevated, `SHOpenFolderAndSelectItems` and an
/// in-process `explorer /select,` call typically open the folder but leave
/// the item unselected (#202). In that case the request is issued through
/// the unelevated Explorer of the current session.
pub fn reveal_item(path: &Path) -> Result<()> {
    let path = shell_path(path)?;
    if !path.exists() {
        return Err(io(
            "reveal in file manager",
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("{} does not exist", path.display()),
            ),
        ));
    }
    if crate::elevation::is_elevated() {
        unelevated_select(&path)
    } else {
        match sta_select(&path) {
            Ok(()) => Ok(()),
            Err(sta_error) => unelevated_select(&path).map_err(|fallback| {
                Error::Unavailable(format!("{sta_error}; unelevated fallback: {fallback}"))
            }),
        }
    }
}

/// Prefer a path form Explorer's PIDL helpers accept: no `\\?\` prefix.
fn shell_path(path: &Path) -> Result<PathBuf> {
    let path = path
        .canonicalize()
        .map_err(|error| io("resolve path for reveal", error))?;
    let text = path.to_string_lossy();
    if let Some(rest) = text.strip_prefix(r"\\?\") {
        if let Some(unc) = rest.strip_prefix(r"UNC\") {
            Ok(PathBuf::from(format!(r"\\{unc}")))
        } else {
            Ok(PathBuf::from(rest))
        }
    } else {
        Ok(path)
    }
}

/// Explorer `/select,` switch, path quoted so spaces survive.
fn select_switch(path: &Path) -> String {
    format!("/select,\"{}\"", path.display())
}

fn sta_select(path: &Path) -> Result<()> {
    let path = path.to_owned();
    let (send, receive) = std::sync::mpsc::channel();
    thread::Builder::new()
        .name("oflh-reveal-sta".into())
        .spawn(move || {
            // SAFETY: this is a new thread with no prior COM apartment.
            let hr = unsafe { CoInitializeEx(null(), COINIT_APARTMENTTHREADED as u32) };
            let result = if hr < 0 {
                Err(io(
                    "initialize COM for reveal",
                    std::io::Error::from_raw_os_error(hr),
                ))
            } else {
                let result = sh_open_and_select(&path);
                if hr == 0 {
                    // SAFETY: this thread initialized COM with S_OK.
                    unsafe { CoUninitialize() };
                }
                result
            };
            let _ = send.send(result);
        })
        .map_err(|error| io("start reveal thread", error))?;
    receive
        .recv_timeout(Duration::from_secs(15))
        .map_err(|_| Error::Unavailable("reveal in file manager timed out".into()))?
}

fn sh_open_and_select(path: &Path) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| Error::Unavailable("path has no containing folder".into()))?;
    let folder = Pidl::new(parent)?;
    let item = Pidl::new(path)?;
    let items = [item.0];
    // SAFETY: both PIDLs stay owned for the call, and `items` holds a valid pointer array.
    let hr = unsafe { SHOpenFolderAndSelectItems(folder.0.cast(), 1, items.as_ptr().cast(), 0) };
    if hr < 0 {
        Err(io(
            "select item in file manager",
            std::io::Error::from_raw_os_error(hr),
        ))
    } else {
        Ok(())
    }
}

struct Pidl(*mut core::ffi::c_void);
impl Pidl {
    fn new(path: &Path) -> Result<Self> {
        let wide = wide(path)?;
        // SAFETY: `wide` is a NUL-terminated path that lives through the call.
        let pidl = unsafe { ILCreateFromPathW(wide.as_ptr()) };
        if pidl.is_null() {
            Err(Error::Unavailable(format!(
                "could not build a shell item for {}",
                path.display()
            )))
        } else {
            Ok(Self(pidl.cast()))
        }
    }
}
impl Drop for Pidl {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: Pidl owns a non-null ILCreateFromPathW allocation.
            unsafe { ILFree(self.0.cast()) };
        }
    }
}

fn unelevated_select(path: &Path) -> Result<()> {
    let explorer = explorer_pid()?;
    let process = open(explorer, PROCESS_QUERY_INFORMATION | PROCESS_DUP_HANDLE)?;
    let mut token = std::ptr::null_mut();
    // SAFETY: `process` is a live process handle; `token` receives an owned handle on success.
    if unsafe { OpenProcessToken(process.as_raw(), TOKEN_DUPLICATE | TOKEN_QUERY, &mut token) } == 0
    {
        return Err(io(
            "open Explorer token for unelevated reveal",
            std::io::Error::last_os_error(),
        ));
    }
    let token = Handle::new(token, "open Explorer token for unelevated reveal")?;
    let mut primary = std::ptr::null_mut();
    // SAFETY: `token` is a valid token; `primary` receives a new primary token.
    if unsafe {
        DuplicateTokenEx(
            token.as_raw(),
            TOKEN_QUERY
                | TOKEN_DUPLICATE
                | TOKEN_ASSIGN_PRIMARY
                | TOKEN_ADJUST_DEFAULT
                | TOKEN_ADJUST_SESSIONID,
            null(),
            SecurityImpersonation,
            TokenPrimary,
            &mut primary,
        )
    } == 0
    {
        return Err(io(
            "duplicate Explorer token for unelevated reveal",
            std::io::Error::last_os_error(),
        ));
    }
    let primary = Handle::new(primary, "duplicate Explorer token for unelevated reveal")?;
    let exe = explorer_executable()?;
    let exe_wide = wide(&exe)?;
    let mut command: Vec<u16> = format!("{} {}", exe.display(), select_switch(path))
        .encode_utf16()
        .chain(Some(0))
        .collect();
    // SAFETY: zero is a valid initial state for these Win32 structures; sizes are set below.
    let mut startup: STARTUPINFOW = unsafe { std::mem::zeroed() };
    startup.cb = size_of::<STARTUPINFOW>() as u32;
    startup.dwFlags = STARTF_USESHOWWINDOW;
    startup.wShowWindow = SW_SHOWNORMAL as u16;
    let mut info: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };
    // SAFETY: `primary` is a primary token from Explorer, the application and
    // command-line buffers are NUL-terminated and live, and `info` is writable.
    let launched = unsafe {
        CreateProcessWithTokenW(
            primary.as_raw(),
            LOGON_WITH_PROFILE,
            exe_wide.as_ptr(),
            command.as_mut_ptr(),
            0,
            null(),
            null(),
            &startup,
            &mut info,
        )
    };
    if launched == 0 {
        return Err(io(
            "start unelevated Explorer reveal",
            std::io::Error::last_os_error(),
        ));
    }
    // SAFETY: CreateProcessWithTokenW transferred process and thread handles to us.
    let _process = Handle::new(info.hProcess, "unelevated Explorer process")?;
    let _thread = Handle::new(info.hThread, "unelevated Explorer thread")?;
    Ok(())
}

fn explorer_executable() -> Result<PathBuf> {
    let windir = std::env::var_os("WINDIR").unwrap_or_else(|| OsString::from(r"C:\Windows"));
    let exe = Path::new(&windir).join("explorer.exe");
    if exe.is_file() {
        Ok(exe)
    } else {
        Err(Error::Unavailable(format!(
            "Explorer was not found at {}",
            exe.display()
        )))
    }
}

fn explorer_pid() -> Result<u32> {
    let class: Vec<u16> = "Shell_TrayWnd".encode_utf16().chain(Some(0)).collect();
    // SAFETY: `class` is a NUL-terminated window class name; the title is unused.
    let hwnd = unsafe { FindWindowW(class.as_ptr(), null()) };
    if !hwnd.is_null() {
        let mut pid = 0;
        // SAFETY: `hwnd` is a live window; `pid` is writable.
        unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
        if pid != 0 {
            return Ok(pid);
        }
    }
    first_explorer_in_session()
}

fn first_explorer_in_session() -> Result<u32> {
    let ours = session_of(unsafe { GetCurrentProcessId() })?;
    let (handle, mut entry) = process_snapshot()?;
    // SAFETY: initialized structure size and live owned snapshot handle.
    let mut next = unsafe { Process32FirstW(handle.0, &mut entry) };
    while next != 0 {
        let name = path(&entry.szExeFile);
        if name
            .file_name()
            .is_some_and(|name| name.eq_ignore_ascii_case("explorer.exe"))
            && session_of(entry.th32ProcessID).ok() == Some(ours)
        {
            return Ok(entry.th32ProcessID);
        }
        // SAFETY: same live snapshot and correctly sized output record.
        next = unsafe { Process32NextW(handle.0, &mut entry) };
    }
    Err(Error::Unavailable(
        "no Explorer process is running in this session".into(),
    ))
}

fn session_of(pid: u32) -> Result<u32> {
    let mut session = 0;
    // SAFETY: `session` is a writable DWORD for this process id.
    if unsafe { ProcessIdToSessionId(pid, &mut session) } == 0 {
        Err(io("read process session", std::io::Error::last_os_error()))
    } else {
        Ok(session)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn select_switch_quotes_paths_that_contain_spaces() {
        assert_eq!(
            select_switch(Path::new(r"C:\Users\alice\My Files\alpha.txt")),
            r#"/select,"C:\Users\alice\My Files\alpha.txt""#
        );
    }
}
