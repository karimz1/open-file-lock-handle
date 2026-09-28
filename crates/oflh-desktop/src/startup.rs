//! Linux WebKit compatibility policy, applied before GTK or application threads start.

/// Apply the Linux renderer policy before creating a Tauri builder.
///
/// WebKitGTK's DMABUF renderer can fail before a window becomes usable on Wayland
/// and NVIDIA systems. An explicitly supplied value always wins, including `0`.
/// On affected configurations we replace the process with the same executable and
/// arguments, setting the child environment through `Command` rather than mutating
/// the environment of a potentially multithreaded process. The supplied variable
/// prevents a relaunch loop. Other operating systems are untouched.
pub fn configure_renderer() -> oflh_core::Result<()> {
    #[cfg(target_os = "linux")]
    {
        use std::{ffi::OsStr, os::unix::process::CommandExt, process::Command};
        let explicit = std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_some();
        let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some_and(|value| !value.is_empty())
            || std::env::var_os("XDG_SESSION_TYPE").as_deref() == Some(OsStr::new("wayland"));
        let nvidia = std::path::Path::new("/sys/module/nvidia/version").exists()
            || std::path::Path::new("/proc/driver/nvidia/version").exists();
        if needs_compatibility(explicit, wayland, nvidia) {
            let executable = std::env::current_exe().map_err(|error| {
                oflh_core::io("resolve desktop executable for renderer setup", error)
            })?;
            let error = Command::new(executable)
                .args(std::env::args_os().skip(1))
                .env("WEBKIT_DISABLE_DMABUF_RENDERER", "1")
                .exec();
            return Err(oflh_core::io(
                "restart desktop with the compatibility renderer",
                error,
            ));
        }
    }
    Ok(())
}

#[cfg(any(target_os = "linux", test))]
fn needs_compatibility(explicit: bool, wayland: bool, nvidia: bool) -> bool {
    !explicit && (wayland || nvidia)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wayland_and_nvidia_use_compatibility_without_overriding_user_choice() {
        for wayland in [false, true] {
            for nvidia in [false, true] {
                assert!(!needs_compatibility(true, wayland, nvidia));
                assert_eq!(
                    needs_compatibility(false, wayland, nvidia),
                    wayland || nvidia
                );
            }
        }
    }
}
