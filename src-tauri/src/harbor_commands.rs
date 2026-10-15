use crate::{config, model::Snapshot, scanner::Scanner};
use tauri::{Manager, State};

#[tauri::command]
pub async fn rescan_now(scanner: State<'_, Scanner>) -> Result<Snapshot, String> {
    scanner.rescan().await
}

#[tauri::command]
pub async fn service_action(
    id: String,
    action: String,
    scanner: State<'_, Scanner>,
) -> Result<(), String> {
    let action = action.parse::<crate::operations::ServiceAction>()?;
    let snapshot = scanner.snapshot()?;
    tauri::async_runtime::spawn_blocking(move || crate::operations::apply(&snapshot, &id, action))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn get_settings() -> Result<config::Config, String> {
    config::load()
}

#[tauri::command]
pub fn ignore_unmanaged(
    key: String,
    ignored: bool,
    scanner: State<'_, Scanner>,
) -> Result<(), String> {
    if !scanner.snapshot()?.unmanaged.iter().any(|g| g.key == key) {
        return Err("Process group no longer exists".into());
    }
    config::set_temporary(&key, ignored)
}

#[tauri::command]
pub fn pin_service(id: String, pinned: bool, scanner: State<'_, Scanner>) -> Result<(), String> {
    let service = scanner
        .snapshot()?
        .services
        .into_iter()
        .find(|s| s.id == id)
        .ok_or("Service no longer exists")?;
    config::update(|c| {
        let entry = c
            .service
            .entry(id)
            .or_insert_with(|| config::ServiceConfig {
                labels: service.jobs.iter().map(|j| j.label.clone()).collect(),
                ..Default::default()
            });
        entry.pinned = pinned;
    })
    .map(|_| ())
}

#[tauri::command]
pub async fn archive_plist(path: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::operations::archive_plist(&path, false).map(|_| ())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    let parsed = reqwest::Url::parse(&url).map_err(|e| e.to_string())?;
    if !["http", "https"].contains(&parsed.scheme()) {
        return Err("Only HTTP(S) URLs can be opened".into());
    }
    std::process::Command::new("/usr/bin/open")
        .arg(url)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn show_main(app: tauri::AppHandle, view: Option<String>) -> Result<(), String> {
    use tauri::Emitter;
    let window = app
        .get_webview_window("main")
        .ok_or("Main window unavailable")?;
    window.show().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())?;
    if let Some(view) = view {
        window
            .emit("harbor://navigate", view)
            .map_err(|e| e.to_string())?;
    }
    if let Some(popover) = app.get_webview_window("popover") {
        let _ = popover.hide();
    }
    Ok(())
}
