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
