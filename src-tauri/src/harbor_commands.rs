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
    if !["start", "stop", "restart"].contains(&action.as_str()) {
        return Err("Unknown service action".into());
    }
    let snapshot = scanner.snapshot()?;
    let mut service = snapshot
        .services
        .into_iter()
        .find(|s| s.id == id)
        .ok_or("Service no longer exists")?;
    if service.jobs.iter().any(|j| !j.editable) {
        return Err("This service includes read-only jobs".into());
    }
    for job in &service.jobs {
        let path = job.plist_path.as_ref().ok_or("Job has no plist")?;
        config::user_plist(path)?;
        if !config::label_valid(&job.label)
            || crate::plist_util::parse_plist(path)
                .map_err(|e| e.to_string())?
                .label
                != job.label
        {
            return Err("Job label changed; rescan first".into());
        }
    }
    service.jobs.sort_by_key(|j| {
        if j.role.contains("postgres") {
            0
        } else if j.role.contains("tunnel") || j.role.contains("cloudflared") {
            2
        } else {
            1
        }
    });
    tauri::async_runtime::spawn_blocking(move || {
        if action != "start" {
            let loaded = crate::launchctl::list_loaded().map_err(|e| e.to_string())?;
            for job in service.jobs.iter().rev() {
                if loaded.iter().any(|l| l.label == job.label) {
                    crate::launchctl::bootout(job.plist_path.as_deref().unwrap())
                        .map_err(|e| format!("{}: {e}", job.label))?;
                }
            }
        }
        if action != "stop" {
            let loaded = crate::launchctl::list_loaded().map_err(|e| e.to_string())?;
            for job in &service.jobs {
                if !loaded.iter().any(|l| l.label == job.label) {
                    crate::launchctl::bootstrap(job.plist_path.as_deref().unwrap())
                        .map_err(|e| format!("{}: {e}", job.label))?;
                }
            }
        }
        Ok(())
    })
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
    config::update(|c| {
        c.ignored.retain(|k| k != &key);
        if ignored {
            c.ignored.push(key);
        }
    })
    .map(|_| ())
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
        let source = config::user_plist(&path)?;
        let plist = crate::plist_util::parse_plist(&path).map_err(|e| e.to_string())?;
        if crate::launchctl::list_loaded()
            .map_err(|e| e.to_string())?
            .iter()
            .any(|j| j.label == plist.label)
        {
            return Err("Only unloaded agents can be archived".into());
        }
        let archive = crate::plist_util::get_user_agents_dir().join(".harbor-archive");
        std::fs::create_dir_all(&archive).map_err(|e| e.to_string())?;
        let destination = archive.join(source.file_name().ok_or("missing filename")?);
        if destination.exists() {
            return Err("An archived plist already has that name".into());
        }
        std::fs::rename(source, destination).map_err(|e| e.to_string())
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
