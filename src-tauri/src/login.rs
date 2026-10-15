use crate::{config, scanner::Scanner};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginApp {
    pub name: String,
    pub path: String,
    pub enabled: bool,
    pub running: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginItems {
    pub apps: Vec<LoginApp>,
    pub extensions: String,
}
#[derive(Default)]
pub struct LoginCache(pub Mutex<Option<LoginItems>>);

fn applescript(script: &str) -> Result<String, String> {
    let output = crate::scan_command::output_with_timeout(
        "/usr/bin/osascript", &["-l", "JavaScript", "-e", script], std::time::Duration::from_secs(30)
    ).map_err(|e| format!("System Events did not finish: {e}. Check Automation permission, then refresh login items before retrying a change."))?;
    if !output.status.success() {
        return Err(format!(
            "Login items could not be accessed. Allow Harbor → System Events in System Settings → Privacy & Security → Automation, then retry. {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().into())
}

pub fn read(processes: &[crate::model::ProcInfo]) -> Result<LoginItems, String> {
    let text = applescript(
        "JSON.stringify(Application('System Events').loginItems().map(x => ({name:x.name(),path:x.path()})))",
    )?;
    let entries: Vec<serde_json::Value> = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let mut known = config::load()?.login_paths;
    for entry in &entries {
        if let (Some(path), Some(name)) = (entry["path"].as_str(), entry["name"].as_str()) {
            known.insert(path.into(), name.into());
        }
    }
    config::update(|c| c.login_paths = known.clone())?;
    let apps = known
        .into_iter()
        .map(|(path, name)| LoginApp {
            enabled: entries
                .iter()
                .any(|e| e["path"].as_str() == Some(path.as_str())),
            running: processes.iter().any(|p| {
                p.argv
                    .first()
                    .is_some_and(|a| a.starts_with(&format!("{path}/")))
            }),
            path,
            name,
        })
        .collect();
    let extensions = crate::scan_command::output("/usr/bin/systemextensionsctl", &["list"])
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_else(|e| format!("Extensions unavailable: {e}"));
    Ok(LoginItems { apps, extensions })
}

#[tauri::command]
pub async fn list_login_items(
    scanner: State<'_, Scanner>,
    cache: State<'_, LoginCache>,
) -> Result<LoginItems, String> {
    let processes = scanner.snapshot()?.processes;
    let items = tauri::async_runtime::spawn_blocking(move || read(&processes))
        .await
        .map_err(|e| e.to_string())??;
    *cache.0.lock().map_err(|e| e.to_string())? = Some(items.clone());
    Ok(items)
}

#[tauri::command]
pub async fn set_login_item(
    path: String,
    enabled: bool,
    cache: State<'_, LoginCache>,
) -> Result<(), String> {
    let app = std::path::PathBuf::from(&path);
    if !app.is_absolute()
        || app.extension().and_then(|s| s.to_str()) != Some("app")
        || !app.is_dir()
    {
        return Err("Choose an existing absolute .app bundle path".into());
    }
    let name = app
        .file_stem()
        .ok_or("Invalid app path")?
        .to_string_lossy()
        .into_owned();
    let quoted = serde_json::to_string(&path).map_err(|e| e.to_string())?;
    let script = format!(
        "const se=Application('System Events');const path={quoted};const items=se.loginItems().filter(x=>x.path()===path);{}",
        if enabled {
            "if(!items.length) se.loginItems.push(se.LoginItem({path:path,hidden:false}));"
        } else {
            "items.forEach(x=>x.delete());"
        }
    );
    tauri::async_runtime::spawn_blocking(move || applescript(&script))
        .await
        .map_err(|e| e.to_string())??;
    config::update(|c| {
        c.login_paths.insert(path, name);
    })?;
    *cache.0.lock().map_err(|e| e.to_string())? = None;
    Ok(())
}
