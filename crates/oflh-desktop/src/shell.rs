//! Tauri transport and native desktop integrations. Inspection remains in the service.
use crate::{contract::*, dropped_target::DroppedTarget, service::Service};
use serde::Serialize;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use tauri::{Emitter, Manager, State};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

type Desktop<'a> = State<'a, Arc<Service>>;
fn integration(error: impl std::error::Error + 'static) -> Failure {
    Failure {
        kind: "desktop_integration".into(),
        message: oflh_core::safe(&error.to_string()),
        os_code: None,
        details: Some(diagnostic_details(&crate::contract::error_chain(&error))),
    }
}

fn diagnostic_details(error: &str) -> String {
    crate::contract::safe_diagnostic(&format!(
        "{error}\n\nRust backtrace:\n{}",
        std::backtrace::Backtrace::force_capture()
    ))
}

fn reveal_with_fallback(
    path: &Path,
    containing: bool,
    mut open_path: impl FnMut(&Path) -> Result<(), String>,
    mut reveal_item: impl FnMut(&Path) -> Result<(), String>,
) -> Result<(), Failure> {
    let parent = path.parent();
    if containing && parent.is_none() {
        return Err(Failure::invalid("No containing folder is available"));
    }
    let (primary, primary_label) = if containing {
        (open_path(parent.unwrap_or(path)), "open containing folder")
    } else {
        (reveal_item(path), "reveal target in file manager")
    };
    let primary_error = match primary {
        Ok(()) => return Ok(()),
        Err(error) => error,
    };
    let fallback = match (containing, parent) {
        (true, Some(parent)) => reveal_item(parent),
        (false, Some(parent)) => open_path(parent),
        (_, None) => Err("The target has no containing folder".into()),
    };
    match fallback {
        Ok(()) => Ok(()),
        Err(fallback_error) => Err(Failure {
            kind: "desktop_integration".into(),
            message: oflh_core::safe(&format!(
                "Could not {primary_label}; the fallback action also failed"
            )),
            os_code: None,
            details: Some(diagnostic_details(&format!(
                "Primary action failed: {primary_error}\nFallback action failed: {fallback_error}"
            ))),
        }),
    }
}
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, Failure> + Send + 'static,
) -> Result<T, Failure> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(integration)?
}
#[tauri::command]
fn status(service: Desktop<'_>) -> Status {
    service.status()
}
#[tauri::command]
fn inspect(service: Desktop<'_>, path: String) -> Result<Status, Failure> {
    service.inspect(PathBuf::from(path))
}
#[tauri::command]
fn inspect_dropped(
    service: Desktop<'_>,
    dropped: State<'_, DroppedTarget>,
    request: u32,
) -> Result<Status, Failure> {
    service.inspect(dropped.take(request)?)
}
#[tauri::command]
fn refresh(service: Desktop<'_>) -> Result<Status, Failure> {
    service.refresh()
}
#[tauri::command]
fn inspect_ports(service: Desktop<'_>) -> Result<Status, Failure> {
    service.ports()
}
#[tauri::command]
fn follow_process(service: Desktop<'_>, revision: u32, key: String) -> Result<Status, Failure> {
    service.follow_process(revision, &key)
}
#[tauri::command]
fn cancel(service: Desktop<'_>) -> Status {
    service.cancel()
}
#[tauri::command]
async fn choose(
    app: tauri::AppHandle,
    service: Desktop<'_>,
    folder: bool,
) -> Result<Option<Status>, Failure> {
    let service = service.inner().clone();
    blocking(move || {
        let picker = app.dialog().file();
        let path = if folder {
            picker.blocking_pick_folder()
        } else {
            picker.blocking_pick_file()
        };
        path.map(|path| service.inspect(path.into_path().map_err(integration)?))
            .transpose()
    })
    .await
}
#[tauri::command]
async fn page(service: Desktop<'_>, revision: u32, query: TableQuery) -> Result<Page, Failure> {
    let dataset = service.dataset(revision)?;
    blocking(move || dataset.page(&query)).await
}
#[tauri::command]
async fn details(service: Desktop<'_>, revision: u32, key: String) -> Result<Details, Failure> {
    let service = service.inner().clone();
    blocking(move || service.details(revision, &key)).await
}
#[tauri::command]
async fn select_all(
    service: Desktop<'_>,
    revision: u32,
    query: TableQuery,
) -> Result<Vec<String>, Failure> {
    let dataset = service.dataset(revision)?;
    blocking(move || dataset.keys(&query)).await
}
#[tauri::command]
async fn copy(
    app: tauri::AppHandle,
    service: Desktop<'_>,
    revision: u32,
    keys: Vec<String>,
    field: String,
    reference: Option<String>,
) -> Result<(), Failure> {
    let dataset = service.dataset(revision)?;
    blocking(move || {
        let text = dataset.copy_text(&keys, &field, reference.as_deref())?;
        app.clipboard().write_text(text).map_err(integration)
    })
    .await
}
#[tauri::command]
fn copy_diagnostic(app: tauri::AppHandle, text: String) -> Result<(), Failure> {
    app.clipboard().write_text(text).map_err(integration)
}
#[tauri::command]
async fn reveal(
    app: tauri::AppHandle,
    service: Desktop<'_>,
    revision: u32,
    reference: String,
    containing: bool,
) -> Result<(), Failure> {
    let dataset = service.dataset(revision)?;
    blocking(move || {
        let path = dataset.path(&reference)?;
        let open_path = |path: &Path| {
            let text = path.to_str().ok_or_else(|| {
                "This native path cannot be passed losslessly to the desktop opener".to_string()
            })?;
            app.opener()
                .open_path(text, None::<&str>)
                .map_err(|error| format!("{error}\n{error:?}"))
        };
        let reveal_item = |path: &Path| {
            app.opener()
                .reveal_item_in_dir(path)
                .map_err(|error| format!("{error}\n{error:?}"))
        };
        reveal_with_fallback(&path, containing, open_path, reveal_item)
    })
    .await
}
#[tauri::command]
async fn prepare(
    service: Desktop<'_>,
    revision: u32,
    keys: Vec<String>,
    force: bool,
) -> Result<Confirmation, Failure> {
    let service = service.inner().clone();
    blocking(move || service.prepare(revision, &keys, force)).await
}
#[tauri::command]
fn prepare_ancestor(
    service: Desktop<'_>,
    owner: String,
    key: String,
    force: bool,
) -> Result<Confirmation, Failure> {
    service.prepare_ancestor(&owner, &key, force)
}
#[tauri::command]
fn prepare_elevated(service: Desktop<'_>, ticket: String) -> Result<Confirmation, Failure> {
    service.prepare_elevated(&ticket)
}
#[tauri::command]
fn dismiss(service: Desktop<'_>) {
    service.dismiss();
}
#[tauri::command]
async fn terminate(service: Desktop<'_>, ticket: String) -> Result<Vec<ActionResult>, Failure> {
    let service = service.inner().clone();
    blocking(move || {
        let mut backend = oflh_platform::native().map_err(Failure::from)?;
        service.terminate(&ticket, &mut *backend)
    })
    .await
}
#[derive(Serialize)]
struct Recent {
    id: u32,
    display: String,
}
#[tauri::command]
fn recent(service: Desktop<'_>) -> Vec<Recent> {
    service
        .recent()
        .into_iter()
        .map(|(id, display)| Recent { id, display })
        .collect()
}
#[tauri::command]
fn remove_recent(service: Desktop<'_>, id: u32) -> Result<(), Failure> {
    service.remove_recent(id)
}
#[tauri::command]
fn clear_recent(service: Desktop<'_>) -> Result<(), Failure> {
    service.clear_recent()
}
#[tauri::command]
fn revisit(service: Desktop<'_>, id: u32) -> Result<Status, Failure> {
    service.revisit(id)
}

#[tauri::command]
fn open_paypal(app: tauri::AppHandle) -> Result<(), Failure> {
    app.opener()
        .open_url(
            "https://www.paypal.com/paypalme/KarimZouine972",
            None::<&str>,
        )
        .map_err(integration)
}

#[tauri::command]
fn open_project(app: tauri::AppHandle) -> Result<(), Failure> {
    app.opener()
        .open_url(
            "https://github.com/karimz1/open-file-lock-handle",
            None::<&str>,
        )
        .map_err(integration)
}

#[tauri::command]
fn open_build(app: tauri::AppHandle) -> Result<(), Failure> {
    if oflh_core::BUILD_URL.is_empty() {
        return Err(Failure::invalid(
            "This build has no GitHub Actions run link",
        ));
    }
    app.opener()
        .open_url(oflh_core::BUILD_URL, None::<&str>)
        .map_err(integration)
}

#[tauri::command]
fn open_pull_request(app: tauri::AppHandle) -> Result<(), Failure> {
    if oflh_core::PULL_REQUEST_URL.is_empty() {
        return Err(Failure::invalid("This build has no pull request link"));
    }
    app.opener()
        .open_url(oflh_core::PULL_REQUEST_URL, None::<&str>)
        .map_err(integration)
}

#[tauri::command]
fn open_profile(app: tauri::AppHandle) -> Result<(), Failure> {
    app.opener()
        .open_url("https://www.karimzouine.com/", None::<&str>)
        .map_err(integration)
}

#[tauri::command]
fn open_donation(app: tauri::AppHandle) -> Result<(), Failure> {
    app.opener()
        .open_url("https://buymeacoffee.com/karimz1", None::<&str>)
        .map_err(integration)
}

#[tauri::command]
fn open_sponsors(app: tauri::AppHandle) -> Result<(), Failure> {
    app.opener()
        .open_url("https://github.com/sponsors/karimz1", None::<&str>)
        .map_err(integration)
}

#[derive(Serialize)]
struct SystemInfo {
    os: &'static str,
    arch: &'static str,
}

#[tauri::command]
fn system_info() -> SystemInfo {
    SystemInfo {
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
    }
}

#[tauri::command]
fn update_mode() -> &'static str {
    if cfg!(target_os = "linux") {
        "download"
    } else {
        "install"
    }
}

#[tauri::command]
fn open_download(app: tauri::AppHandle) -> Result<(), Failure> {
    app.opener()
        .open_url("https://oflh.karimzouine.com/#download", None::<&str>)
        .map_err(integration)
}

fn installed_release_url(version: &str) -> Result<url::Url, url::ParseError> {
    let mut url = url::Url::parse("https://github.com/karimz1/open-file-lock-handle/releases")?;
    url.set_fragment(Some(&format!("release-v{version}")));
    Ok(url)
}

#[tauri::command]
fn open_installed_release(app: tauri::AppHandle) -> Result<(), Failure> {
    let url = installed_release_url(oflh_core::display_version()).map_err(integration)?;
    app.opener()
        .open_url(url.as_str(), None::<&str>)
        .map_err(integration)
}

#[tauri::command]
fn open_release_notes(app: tauri::AppHandle) -> Result<(), Failure> {
    app.opener()
        .open_url(
            "https://github.com/karimz1/open-file-lock-handle/releases/latest",
            None::<&str>,
        )
        .map_err(integration)
}

#[tauri::command]
fn open_issue(app: tauri::AppHandle, title: String, body: String) -> Result<(), Failure> {
    let mut url = url::Url::parse("https://github.com/karimz1/open-file-lock-handle/issues/new")
        .map_err(integration)?;
    url.query_pairs_mut()
        .append_pair("title", &title)
        .append_pair("body", &body);
    app.opener()
        .open_url(url.as_str(), None::<&str>)
        .map_err(integration)
}

/// Start an inspection that another launch of the app forwarded to this window.
///
/// The window is brought to the front either way. A target replaces any running
/// inspection, matching what the user just chose in Explorer, and the page reacts
/// as it does to a dropped target.
#[cfg(windows)]
fn take_over_launch(app: &tauri::AppHandle, arguments: Vec<String>, working_directory: String) {
    if let Some(window) = app.get_webview_window("main") {
        // Focus can fail while Windows withholds foreground rights; the scan still starts.
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
    let Some(target) = crate::launch::forwarded_target(arguments, &working_directory) else {
        return;
    };
    let Some(service) = app.try_state::<Arc<Service>>() else {
        return;
    };
    match service.take_over(target) {
        Ok(status) => {
            let _ = app.emit("target-dropped", ());
            let _ = app.emit("scan-status", status);
        }
        Err(error) => {
            let _ = app.emit("desktop-error", error);
        }
    }
}

/// Launch the desktop shell. No terminal UI code is linked into this binary.
///
/// `initial_target` comes from `--inspect <path>` (the Windows Explorer context
/// menu). Its inspection starts before the WebView loads; the page picks it up
/// through `status` like any other running inspection. On Windows, a launch while
/// a window is open exits after handing its arguments to that window
/// ([`take_over_launch`]).
pub fn run(initial_target: Option<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    let builder = tauri::Builder::default();
    // The single-instance plugin must be registered first.
    #[cfg(windows)]
    let builder = builder.plugin(tauri_plugin_single_instance::init(take_over_launch));
    builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let app_data = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_data)?;
            let service = Service::with_recent_database(
                oflh_platform::native()?,
                move |status| {
                    // A closing WebView may no longer receive notifications.
                    let _ = handle.emit("scan-status", status);
                },
                app_data.join("oflh.sqlite3"),
            )?;
            if let Some(target) = initial_target {
                // Only an empty path (excluded by the parser) or an exhausted scan
                // generation counter, impossible in a fresh service, can fail here.
                service.inspect(target)?;
            }
            app.manage(Arc::new(service));
            app.manage(DroppedTarget::default());
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::DragDrop(event) = event {
                if window.state::<Arc<Service>>().status().scanning {
                    return;
                }
                match event {
                    tauri::DragDropEvent::Enter { .. } => {
                        let _ = window.emit("drag-active", true);
                    }
                    tauri::DragDropEvent::Leave => {
                        let _ = window.emit("drag-active", false);
                    }
                    tauri::DragDropEvent::Drop { paths, .. } => {
                        let _ = window.emit("drag-active", false);
                        // The UI must accept this request from a view allowing file drops.
                        // Keep the OS PathBuf in Rust, including non-Unicode filenames.
                        match window.state::<DroppedTarget>().capture(paths) {
                            Ok(request) => {
                                let _ = window.emit("target-drop-requested", request);
                            }
                            Err(error) => {
                                let _ = window.emit("desktop-error", error);
                            }
                        }
                    }
                    _ => {}
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            status,
            open_project,
            open_build,
            open_pull_request,
            open_profile,
            open_donation,
            open_sponsors,
            system_info,
            update_mode,
            open_download,
            open_installed_release,
            open_release_notes,
            open_issue,
            inspect,
            inspect_dropped,
            refresh,
            inspect_ports,
            follow_process,
            cancel,
            choose,
            page,
            details,
            select_all,
            copy,
            copy_diagnostic,
            reveal,
            prepare,
            prepare_ancestor,
            prepare_elevated,
            dismiss,
            terminate,
            recent,
            remove_recent,
            open_paypal,
            clear_recent,
            revisit
        ])
        .run(tauri::generate_context!())?;
    Ok(())
}

#[cfg(test)]
mod reveal_tests {
    use super::*;

    #[test]
    fn installed_version_links_to_its_release_anchor() {
        assert_eq!(
            installed_release_url("0.4.0").unwrap().as_str(),
            "https://github.com/karimz1/open-file-lock-handle/releases#release-v0.4.0"
        );
        assert_eq!(
            update_mode(),
            if cfg!(target_os = "linux") {
                "download"
            } else {
                "install"
            }
        );
    }

    #[test]
    fn reveal_failure_falls_back_to_opening_the_containing_folder() {
        let path = Path::new("/workspace/project/bin/app");
        let mut opened = None;
        let result = reveal_with_fallback(
            path,
            false,
            |folder| {
                opened = Some(folder.to_owned());
                Ok(())
            },
            |_| Err("FileManager1 is unavailable".into()),
        );

        assert!(result.is_ok());
        assert_eq!(opened.as_deref(), Some(Path::new("/workspace/project/bin")));
    }

    #[test]
    fn open_containing_folder_falls_back_to_native_reveal() {
        let path = Path::new("/workspace/project/bin/app");
        let mut revealed = None;
        let result = reveal_with_fallback(
            path,
            true,
            |_| Err("default opener failed".into()),
            |folder| {
                revealed = Some(folder.to_owned());
                Ok(())
            },
        );

        assert!(result.is_ok());
        assert_eq!(
            revealed.as_deref(),
            Some(Path::new("/workspace/project/bin"))
        );
    }

    #[test]
    fn reveal_failure_keeps_primary_and_fallback_diagnostics() {
        let result = reveal_with_fallback(
            Path::new("/workspace/project/bin/app"),
            false,
            |_| Err("xdg-open failed".into()),
            |_| Err("D-Bus unavailable".into()),
        )
        .unwrap_err();

        assert!(result.message.contains("fallback action also failed"));
        let details = result.details.unwrap();
        assert!(details.contains("D-Bus unavailable"));
        assert!(details.contains("xdg-open failed"));
        assert!(details.contains("Rust backtrace"));
    }
}
