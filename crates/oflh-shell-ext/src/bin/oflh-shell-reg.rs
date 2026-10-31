//! Registers or removes the OFLH Desktop Windows 11 context menu identity
//! package through the Windows PackageManager API. The NSIS installer runs it
//! from `<install dir>\shell-ext\`; no PowerShell, script execution policy or
//! Developer Mode is involved.
//!
//! Usage: `oflh-shell-reg register|unregister|status`.
//! Exit codes: 0 done, 2 skipped (not Windows 11 or package missing), 1 failed.
//! Status: 0 registered for this folder, 3 registered for another folder, 2 none.
//! A log is written to `%TEMP%\oflh-context-menu.log`.

#[cfg(not(windows))]
fn main() {
    eprintln!("oflh-shell-reg only runs on Windows");
    std::process::exit(2);
}

#[cfg(windows)]
fn main() {
    std::process::exit(win::run());
}

#[cfg(windows)]
mod win {
    use std::io::Write;
    use std::path::{Path, PathBuf};
    use windows::Foundation::Uri;
    use windows::Management::Deployment::{
        AddPackageOptions, DeploymentResult, PackageManager, RemovalOptions,
    };
    use windows::Win32::System::Registry::{
        HKEY, HKEY_LOCAL_MACHINE, KEY_SET_VALUE, KEY_WOW64_64KEY, REG_OPTION_NON_VOLATILE, REG_SZ,
        RRF_RT_REG_SZ, RRF_SUBKEY_WOW6464KEY, RegCloseKey, RegCreateKeyExW, RegDeleteKeyExW,
        RegDeleteTreeW, RegGetValueW, RegSetValueExW,
    };
    use windows::Win32::UI::Shell::IsUserAnAdmin;
    use windows_core::{HSTRING, PCWSTR};

    const NAME: &str = "KarimZouine.OFLHDesktop.ContextMenu";
    const PUBLISHER: &str = "CN=Karim Zouine, OID.2.25.311729368913984317654407730594956997722=1";
    // InstallLocation of a package with external location is its WindowsApps
    // folder, so the external location is remembered here (machine-wide, like
    // the unsigned package; readable without admin rights, 64-bit view).
    const STATE_KEY: &str = "SOFTWARE\\KarimZouine\\OFLH Desktop";
    const STATE_PARENT: &str = "SOFTWARE\\KarimZouine";
    const STATE_VALUE: &str = "ContextMenuExternalLocation";

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    fn log(action: &str, text: &str) {
        let path = std::env::temp_dir().join("oflh-context-menu.log");
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            let _ = writeln!(f, "[{action}] {text}");
        }
        println!("{text}");
    }

    fn is_admin() -> bool {
        // SAFETY: no arguments, no preconditions.
        unsafe { IsUserAnAdmin().as_bool() }
    }

    fn registered(pm: &PackageManager) -> windows_core::Result<Vec<HSTRING>> {
        let name = HSTRING::from(NAME);
        let publisher = HSTRING::from(PUBLISHER);
        let packages = if is_admin() {
            pm.FindPackagesByNamePublisher(&name, &publisher)?
        } else {
            pm.FindPackagesByUserSecurityIdNamePublisher(&HSTRING::new(), &name, &publisher)?
        };
        let mut names = Vec::new();
        for p in packages {
            names.push(p.Id()?.FullName()?);
        }
        Ok(names)
    }

    fn get_state() -> Option<String> {
        let key = wide(STATE_KEY);
        let value = wide(STATE_VALUE);
        let mut buf = [0u16; 1024];
        let mut size = (buf.len() * 2) as u32;
        // SAFETY: NUL-terminated names; buf and size describe a valid buffer.
        let rc = unsafe {
            RegGetValueW(
                HKEY_LOCAL_MACHINE,
                PCWSTR(key.as_ptr()),
                PCWSTR(value.as_ptr()),
                RRF_RT_REG_SZ | RRF_SUBKEY_WOW6464KEY,
                None,
                Some(buf.as_mut_ptr().cast()),
                Some(&mut size),
            )
        };
        if rc.is_err() {
            return None;
        }
        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        Some(String::from_utf16_lossy(&buf[..len]))
    }

    fn set_state(location: &str) -> windows_core::Result<()> {
        let key = wide(STATE_KEY);
        let value = wide(STATE_VALUE);
        let data = wide(location);
        let mut hkey = HKEY::default();
        // SAFETY: NUL-terminated strings that outlive the calls; hkey is closed.
        unsafe {
            RegCreateKeyExW(
                HKEY_LOCAL_MACHINE,
                PCWSTR(key.as_ptr()),
                None,
                PCWSTR::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_SET_VALUE | KEY_WOW64_64KEY,
                None,
                &mut hkey,
                None,
            )
            .ok()?;
            let bytes = std::slice::from_raw_parts(data.as_ptr().cast::<u8>(), data.len() * 2);
            let rc = RegSetValueExW(hkey, PCWSTR(value.as_ptr()), None, REG_SZ, Some(bytes));
            let _ = RegCloseKey(hkey);
            rc.ok()
        }
    }

    fn clear_state() {
        let key = wide(STATE_KEY);
        let parent = wide(STATE_PARENT);
        // SAFETY: NUL-terminated key names that outlive the calls. The parent
        // is only removed when empty (RegDeleteKeyExW fails otherwise).
        unsafe {
            let _ = RegDeleteTreeW(HKEY_LOCAL_MACHINE, PCWSTR(key.as_ptr()));
            let _ = RegDeleteKeyExW(
                HKEY_LOCAL_MACHINE,
                PCWSTR(key.as_ptr()),
                KEY_WOW64_64KEY.0,
                None,
            );
            let _ = RegDeleteKeyExW(
                HKEY_LOCAL_MACHINE,
                PCWSTR(parent.as_ptr()),
                KEY_WOW64_64KEY.0,
                None,
            );
        }
    }

    fn file_uri(path: &Path) -> windows_core::Result<Uri> {
        Uri::CreateUri(&HSTRING::from(path.as_os_str()))
    }

    fn check(result: windows_core::Result<DeploymentResult>) -> windows_core::Result<()> {
        let result = result?;
        let code = result.ExtendedErrorCode()?;
        if code.is_err() {
            return Err(windows_core::Error::new(
                code,
                result.ErrorText()?.to_string(),
            ));
        }
        Ok(())
    }

    fn remove_all(pm: &PackageManager, action: &str) -> windows_core::Result<()> {
        for full in registered(pm)? {
            log(action, &format!("Removing {full}"));
            let op = if is_admin() {
                pm.RemovePackageWithOptionsAsync(&full, RemovalOptions::RemoveForAllUsers)?
            } else {
                pm.RemovePackageAsync(&full)?
            };
            check(op.get())?;
        }
        Ok(())
    }

    fn os_build() -> u32 {
        // RtlGetVersion is not affected by manifest-based version lies.
        let mut info = windows::Win32::System::SystemInformation::OSVERSIONINFOW {
            dwOSVersionInfoSize: std::mem::size_of::<
                windows::Win32::System::SystemInformation::OSVERSIONINFOW,
            >() as u32,
            ..Default::default()
        };
        // SAFETY: info is a valid, sized OSVERSIONINFOW.
        unsafe {
            let _ = windows::Wdk::System::SystemServices::RtlGetVersion(&mut info);
        }
        info.dwBuildNumber
    }

    pub fn run() -> i32 {
        let action = std::env::args()
            .nth(1)
            .unwrap_or_else(|| "status".into())
            .to_ascii_lowercase();
        let exe = std::env::current_exe().unwrap_or_default();
        let here = exe.parent().map(Path::to_path_buf).unwrap_or_default();
        let install_dir: PathBuf = here.parent().map(Path::to_path_buf).unwrap_or_default();
        let install = install_dir
            .to_string_lossy()
            .trim_end_matches('\\')
            .to_string();
        let result = (|| -> windows_core::Result<i32> {
            let pm = PackageManager::new()?;
            match action.as_str() {
                "status" => {
                    let packages = registered(&pm)?;
                    if packages.is_empty() {
                        return Ok(2);
                    }
                    for p in &packages {
                        println!("{p}");
                    }
                    let location = get_state();
                    println!("External location: {}", location.as_deref().unwrap_or(""));
                    let same = location
                        .is_some_and(|l| l.trim_end_matches('\\').eq_ignore_ascii_case(&install));
                    Ok(if same { 0 } else { 3 })
                }
                "register" => {
                    if os_build() < 22000 {
                        log(&action, "Not Windows 11; classic menu only.");
                        return Ok(2);
                    }
                    let msix = here.join("oflh-context-menu.msix");
                    if !msix.exists() {
                        log(&action, &format!("No {}", msix.display()));
                        return Ok(2);
                    }
                    remove_all(&pm, &action)?;
                    // package.ps1 leaves this marker next to packages it did not sign.
                    let signed = !here.join("unsigned").exists();
                    let options = AddPackageOptions::new()?;
                    options.SetExternalLocationUri(&file_uri(&install_dir)?)?;
                    // Windows 11 accepts unsigned packages whose publisher
                    // carries the unsigned-package OID, from an elevated process.
                    options.SetAllowUnsigned(!signed)?;
                    check(pm.AddPackageByUriAsync(&file_uri(&msix)?, &options)?.get())?;
                    set_state(&install)?;
                    log(
                        &action,
                        &format!("Registered (signed: {signed}) for {install}"),
                    );
                    Ok(0)
                }
                "unregister" => {
                    remove_all(&pm, &action)?;
                    clear_state();
                    Ok(0)
                }
                _ => {
                    eprintln!("usage: oflh-shell-reg register|unregister|status");
                    Ok(1)
                }
            }
        })();
        result.unwrap_or_else(|e| {
            log(&action, &format!("Failed: {e}"));
            1
        })
    }
}
