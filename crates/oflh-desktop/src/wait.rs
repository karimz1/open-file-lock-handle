//! Non-destructive waiting actions and desktop notification policy.
use crate::contract::*;

/// Notify only after a validated file/port release or captured process exit.
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

/// Localized desktop toast naming the completed selection and its scope.
pub fn notification_message(locale: &str, target: &WaitTargetView) -> (&'static str, String) {
    let process = target.kind == "process";
    let (title, body) = if locale.starts_with("de") {
        if process {
            (
                "OFLH: Prozess beendet",
                "Der ausgewählte Prozess ist beendet. Andere Prozesse können seine Dateien weiterhin verwenden.",
            )
        } else {
            (
                "OFLH: Ziel ist frei",
                "Keine Nutzer dieses Ziels mehr gefunden.",
            )
        }
    } else if locale.starts_with("zh") {
        if process {
            (
                "OFLH：进程已退出",
                "所选进程已退出。其他进程仍可能使用其文件。",
            )
        } else {
            ("OFLH：目标已空闲", "未发现此目标的占用进程。")
        }
    } else if process {
        (
            "OFLH: Process exited",
            "The selected process has exited. Other processes may still be using its files.",
        )
    } else {
        ("OFLH: Target is free", "No holders of this target remain.")
    };
    (title, format!("{}\n{body}", target.display))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn notification_text_identifies_the_completed_process_and_target() {
        let target = WaitTargetView {
            kind: "process",
            display: "fixture (PID 424242)".into(),
        };
        for locale in ["de-DE", "zh-CN", "en-GB"] {
            assert!(
                notification_message(locale, &target)
                    .1
                    .contains(&target.display)
            );
        }
        assert!(
            notification_message("en", &target)
                .0
                .contains("Process exited")
        );
    }
}
