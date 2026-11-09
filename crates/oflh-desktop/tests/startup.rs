//! Re-exec coverage runs only inside an isolated test child; parent environment is untouched.
#[cfg(target_os = "linux")]
mod linux {
    use std::process::{Command, Stdio};
    #[test]
    fn renderer_child() {
        if std::env::var_os("OFLH_RENDERER_TEST_CHILD").is_none() {
            return;
        }
        oflh_desktop::startup::configure_renderer().unwrap();
        println!(
            "RENDERER {} {}",
            std::process::id(),
            std::env::var("WEBKIT_DISABLE_DMABUF_RENDERER").unwrap()
        );
    }
    #[test]
    fn safe_reexec_preserves_pid_and_respects_explicit_opt_out() {
        for explicit in [None, Some("0"), Some("1")] {
            let mut command = Command::new(std::env::current_exe().unwrap());
            command
                .args(["--exact", "linux::renderer_child", "--nocapture"])
                .env("OFLH_RENDERER_TEST_CHILD", "1")
                .env("WAYLAND_DISPLAY", "oflh-test-wayland")
                .stdout(Stdio::piped());
            if let Some(value) = explicit {
                command.env("WEBKIT_DISABLE_DMABUF_RENDERER", value);
            } else {
                command.env_remove("WEBKIT_DISABLE_DMABUF_RENDERER");
            }
            let child = command.spawn().unwrap();
            let pid = child.id();
            let output = child.wait_with_output().unwrap();
            assert!(output.status.success());
            let text = String::from_utf8(output.stdout).unwrap();
            assert!(text.contains(&format!("RENDERER {pid} {}", explicit.unwrap_or("1"))));
        }
    }
}

#[test]
fn main_window_does_not_depend_on_a_page_load_callback_to_be_visible() {
    let config: serde_json::Value =
        serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
    let window = &config["app"]["windows"][0];
    assert_eq!(window["label"], "main");
    assert_eq!(window["visible"], true);
    assert_eq!(window["backgroundColor"], "#1e1e1e");
}

/// "Open window maximized" is a setting that defaults to off, so the static
/// window config must keep starting at its restore size, and the page needs
/// the permission to maximize the window when the setting is on.
#[test]
fn main_window_starts_at_restore_size_and_may_be_maximized_by_the_setting() {
    let config: serde_json::Value =
        serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
    let window = &config["app"]["windows"][0];
    assert_ne!(window["maximized"], true);
    assert_ne!(window["fullscreen"], true);
    assert_ne!(window["resizable"], false);
    assert_ne!(window["maximizable"], false);
    let capability: serde_json::Value =
        serde_json::from_str(include_str!("../capabilities/main.json")).unwrap();
    let permissions = capability["permissions"].as_array().unwrap();
    assert!(permissions.contains(&"core:window:allow-maximize".into()));
}

/// Tauri decodes the first ICO entry for a running Windows window in both debug
/// and packaged builds. Windows shortcuts can pick another entry, hiding a
/// 16-pixel source that becomes blurred when enlarged for the taskbar or HiDPI.
#[test]
fn windows_window_icon_uses_the_largest_frame_and_retains_small_shell_icons() {
    let config: serde_json::Value =
        serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
    let icon = config["bundle"]["icon"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(serde_json::Value::as_str)
        .find(|path| path.ends_with(".ico"))
        .unwrap();
    let bytes = std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(icon)).unwrap();
    assert!(bytes.len() >= 6);
    assert_eq!(&bytes[..4], &[0, 0, 1, 0], "Expected a Windows ICO file");
    let count = u16::from_le_bytes(bytes[4..6].try_into().unwrap()) as usize;
    assert!(count > 0 && bytes.len() >= 6 + count * 16);
    let dimension = |value: u8| {
        if value == 0 {
            256_u16
        } else {
            u16::from(value)
        }
    };
    let mut sizes = Vec::new();
    for index in 0..count {
        let entry = &bytes[6 + index * 16..22 + index * 16];
        let size = u32::from_le_bytes(entry[8..12].try_into().unwrap()) as usize;
        let offset = u32::from_le_bytes(entry[12..16].try_into().unwrap()) as usize;
        assert!(offset >= 6 + count * 16 && offset <= bytes.len());
        assert!(size <= bytes.len() - offset);
        let dimensions = (dimension(entry[0]), dimension(entry[1]));
        assert_eq!(u16::from_le_bytes(entry[6..8].try_into().unwrap()), 32);
        sizes.push(dimensions);
        if index == 0 {
            assert_eq!(
                dimensions,
                (256, 256),
                "Keep the high-resolution frame first: Tauri uses it for the running window"
            );
            assert_eq!(
                &bytes[offset..offset + size],
                include_bytes!("../icons/128x128@2x.png")
            );
        }
    }
    for size in [16, 24, 32, 48, 64, 256] {
        assert!(
            sizes.contains(&(size, size)),
            "Retain the native {size}px shell icon"
        );
    }
}
