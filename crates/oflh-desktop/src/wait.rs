//! Non-destructive waiting actions and desktop notification policy.
use crate::contract::*;

/// Notify only after a validated file/port release or captured process exit.
pub fn notify_free(
    app: &tauri::AppHandle,
    title: &str,
    body: &str,
    focus: bool,
) -> Result<(), Failure> {
    use tauri::Manager;
    use tauri_plugin_notification::NotificationExt;
    app.notification()
        .builder()
        .title(title)
        .body(body)
        .show()
        .map_err(|error| {
            Failure::invalid(oflh_core::safe(&format!("Show free notification: {error}")))
        })?;
    if let Some(window) = app.get_webview_window("main")
        && focus
    {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
    Ok(())
}
/// Notification text arrives already translated from the UI's locale tables.
/// It is display-only, so control characters are removed and the length is bounded.
pub fn notification_text(text: &str) -> String {
    oflh_core::safe(text).chars().take(512).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn notification_text_is_bounded_and_strips_control_characters() {
        assert_eq!(notification_text("OFLH: Verschoben"), "OFLH: Verschoben");
        assert!(!notification_text("a\u{7}b").contains('\u{7}'));
        assert_eq!(notification_text(&"x".repeat(2000)).chars().count(), 512);
    }
}
