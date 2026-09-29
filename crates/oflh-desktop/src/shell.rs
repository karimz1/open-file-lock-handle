//! Tauri transport and native desktop integrations. Inspection remains in the service.
use crate::{contract::*, service::Service};
use serde::Serialize;
use std::{path::PathBuf, sync::Arc};
use tauri::{Emitter, Manager, State};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

type Desktop<'a> = State<'a, Arc<Service>>;
fn integration(error: impl std::fmt::Display) -> Failure {
    Failure {
        kind: "desktop_integration".into(),
        message: oflh_core::safe(&error.to_string()),
        os_code: None,
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
        if containing {
            let parent = path
                .parent()
                .ok_or_else(|| Failure::invalid("No containing folder is available"))?;
            let text = parent.to_str().ok_or_else(|| {
                Failure::invalid(
                    "This native path cannot be passed losslessly to the desktop opener",
                )
            })?;
            app.opener()
                .open_path(text, None::<&str>)
                .map_err(integration)
        } else {
            app.opener().reveal_item_in_dir(path).map_err(integration)
        }
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
fn revisit(service: Desktop<'_>, id: u32) -> Result<Status, Failure> {
    service.revisit(id)
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
fn open_donation(app: tauri::AppHandle) -> Result<(), Failure> {
    app.opener()
        .open_url("https://buymeacoffee.com/karimz1", None::<&str>)
        .map_err(integration)
}

/// Launch the desktop shell. No terminal UI code is linked into this binary.
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
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
            app.manage(Arc::new(service));
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::DragDrop(event) = event {
                match event {
                    tauri::DragDropEvent::Enter { .. } => {
                        let _ = window.emit("drag-active", true);
                    }
                    tauri::DragDropEvent::Leave => {
                        let _ = window.emit("drag-active", false);
                    }
                    tauri::DragDropEvent::Drop { paths, .. } => {
                        let _ = window.emit("drag-active", false);
                        // Keep the OS PathBuf in Rust, including non-Unicode filenames.
                        if paths.len() == 1 {
                            if let Some(path) = paths.first() {
                                match window.state::<Arc<Service>>().inspect(path.clone()) {
                                    Ok(status) => {
                                        let _ = window.emit("target-dropped", ());
                                        let _ = window.emit("scan-status", status);
                                    }
                                    Err(error) => {
                                        let _ = window.emit("desktop-error", error);
                                    }
                                }
                            }
                        } else {
                            let _ = window.emit(
                                "desktop-error",
                                Failure::invalid("Drop one file or folder at a time"),
                            );
                        }
                    }
                    _ => {}
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            status,
            open_project,
            open_donation,
            inspect,
            refresh,
            inspect_ports,
            follow_process,
            cancel,
            choose,
            page,
            details,
            select_all,
            copy,
            reveal,
            prepare,
            prepare_ancestor,
            dismiss,
            terminate,
            recent,
            revisit
        ])
        .run(tauri::generate_context!())?;
    Ok(())
}
