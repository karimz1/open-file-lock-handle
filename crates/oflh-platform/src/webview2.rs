//! Detect the system WebView2 runtime and keep its profile next to a portable exe.
use oflh_core::{Error, Result, io};
use std::path::Path;

/// Microsoft's Evergreen WebView2 download page, also used by the installer.
pub const DOWNLOAD_URL: &str = "https://developer.microsoft.com/microsoft-edge/webview2/";

const RUNTIME_GUID: &str =
    r"SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";
const RUNTIME_GUID_WOW: &str =
    r"SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";

/// Point WebView2 at `directory` before the WebView is created.
///
/// Must run on the main thread, before Tauri starts worker threads.
pub fn set_user_data_folder(directory: &Path) -> Result<()> {
    std::fs::create_dir_all(directory)
        .map_err(|error| io("create the portable WebView2 data folder", error))?;
    // SAFETY: called from the desktop entry point before the Tauri runtime
    // starts additional threads that could read the environment.
    unsafe {
        std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", directory);
    }
    Ok(())
}

/// Whether a WebView2 runtime version is recorded in the usual registry keys
/// or the default install locations.
pub fn is_installed() -> bool {
    registry_has_runtime() || default_install_dir_has_runtime()
}

/// If the runtime is missing, show a message with the download page and fail so
/// the app does not start a WebView that would crash.
pub fn ensure_installed() -> Result<()> {
    if is_installed() {
        return Ok(());
    }
    if prompt_to_open_download_page() {
        open_download_page();
    }
    Err(Error::Unavailable(
        "OFLH Desktop needs the Microsoft Edge WebView2 Runtime".into(),
    ))
}

fn default_install_dir_has_runtime() -> bool {
    let roots = [
        std::env::var_os("ProgramFiles(x86)"),
        std::env::var_os("ProgramFiles"),
        std::env::var_os("LOCALAPPDATA"),
    ];
    roots.into_iter().flatten().any(|root| {
        let directory = Path::new(&root)
            .join("Microsoft")
            .join("EdgeWebView")
            .join("Application");
        directory.is_dir()
            && std::fs::read_dir(&directory).is_ok_and(|entries| {
                entries
                    .filter_map(|entry| entry.ok())
                    .any(|entry| entry.path().join("msedgewebview2.exe").is_file())
            })
    })
}

fn registry_has_runtime() -> bool {
    // The NSIS installer checks these same keys before embedding the bootstrapper.
    ["HKEY_LOCAL_MACHINE", "HKEY_CURRENT_USER"]
        .into_iter()
        .flat_map(|hive| {
            [RUNTIME_GUID_WOW, RUNTIME_GUID]
                .into_iter()
                .map(move |path| (hive, path))
        })
        .any(|(hive, path)| !registry_version(hive, path).is_empty())
}

fn registry_version(hive: &str, path: &str) -> String {
    let key = format!("{hive}\\{path}");
    let output = std::process::Command::new("reg")
        .args(["query", &key, "/v", "pv"])
        .output();
    let Ok(output) = output else {
        return String::new();
    };
    if !output.status.success() {
        return String::new();
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| {
            let line = line.trim();
            let (_, value) = line.split_once("REG_SZ")?;
            let value = value.trim();
            (!value.is_empty()).then(|| value.to_string())
        })
        .unwrap_or_default()
}

fn prompt_to_open_download_page() -> bool {
    let title: Vec<u16> = "OFLH Desktop\0".encode_utf16().collect();
    let text: Vec<u16> = "OFLH Desktop needs the Microsoft Edge WebView2 Runtime, which is not installed.\n\nDownload page:\nhttps://developer.microsoft.com/microsoft-edge/webview2/\n\nOpen the download page now?\0"
        .encode_utf16()
        .collect();
    // SAFETY: title and text are valid, null-terminated UTF-16; no owner window.
    unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::MessageBoxW(
            std::ptr::null_mut(),
            text.as_ptr(),
            title.as_ptr(),
            windows_sys::Win32::UI::WindowsAndMessaging::MB_ICONEXCLAMATION
                | windows_sys::Win32::UI::WindowsAndMessaging::MB_YESNO,
        ) == windows_sys::Win32::UI::WindowsAndMessaging::IDYES
    }
}

fn open_download_page() {
    let verb: Vec<u16> = "open\0".encode_utf16().collect();
    let url: Vec<u16> = format!("{DOWNLOAD_URL}\0").encode_utf16().collect();
    // SAFETY: verb and url are valid, null-terminated UTF-16 for ShellExecuteW.
    unsafe {
        windows_sys::Win32::UI::Shell::ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            url.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn download_page_matches_the_installer() {
        assert_eq!(
            DOWNLOAD_URL,
            "https://developer.microsoft.com/microsoft-edge/webview2/"
        );
    }
}
