//! Non-destructive waiting actions and desktop notification policy.
use crate::contract::*;

/// Complete the notification side effect only after the confirming free scan.
pub fn notify_free(app: &tauri::AppHandle, title: &str, body: &str) -> Result<(), Failure> {
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
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
    Ok(())
}

/// Localized desktop toast after the worker's confirming free scan.
pub fn notification_message(locale: &str) -> (&'static str, &'static str) {
    if locale.starts_with("de") {
        (
            "OFLH: Ziel ist frei",
            "Keine Nutzer mehr gefunden. Gewählte Folgeaktion wird ausgeführt.",
        )
    } else if locale.starts_with("zh") {
        ("OFLH：目标已空闲", "未发现占用进程。将执行所选的后续操作。")
    } else {
        (
            "OFLH: Target is free",
            "No holders remain. The selected follow-up can now run.",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn notification_text_back_translates_to_free_target_and_follow_up() {
        assert!(notification_message("de-DE").1.contains("Folgeaktion"));
        assert!(notification_message("zh-CN").1.contains("后续操作"));
        assert!(notification_message("en-GB").1.contains("follow-up"));
    }
}
