use serde::Serialize;
use std::{
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
use tauri::{Manager, ipc::Channel};
use tauri_plugin_updater::{Update, UpdaterExt};
use tokio::sync::Mutex;

#[derive(Default)]
pub struct Updates {
    pending: Mutex<Pending>,
    installing: AtomicBool,
}

impl Updates {
    pub fn installing(&self) -> bool {
        self.installing.load(Ordering::SeqCst)
    }
}

#[derive(Default)]
struct Pending {
    update: Option<Update>,
    installed: bool,
}

#[derive(Serialize)]
pub struct Status {
    current_version: String,
    can_install: bool,
}

#[derive(Serialize)]
pub struct Available {
    version: String,
    notes: String,
}

#[derive(Clone, Serialize)]
pub struct Progress {
    stage: &'static str,
    downloaded: u64,
    total: Option<u64>,
}

fn installed_path(exe: &Path, home: &Path) -> bool {
    let Some(macos) = exe.parent() else {
        return false;
    };
    let Some(contents) = macos.parent() else {
        return false;
    };
    let Some(bundle) = contents.parent() else {
        return false;
    };
    macos.file_name().is_some_and(|n| n == "MacOS")
        && contents.file_name().is_some_and(|n| n == "Contents")
        && bundle.extension().is_some_and(|n| n == "app")
        && (bundle.starts_with("/Applications") || bundle.starts_with(home.join("Applications")))
}

fn can_install() -> bool {
    !cfg!(debug_assertions)
        && !cfg!(feature = "smoke-test")
        && std::env::current_exe()
            .ok()
            .zip(dirs::home_dir())
            .is_some_and(|(exe, home)| installed_path(&exe, &home))
}

#[tauri::command]
pub fn update_status(app: tauri::AppHandle) -> Status {
    Status {
        current_version: app.package_info().version.to_string(),
        can_install: can_install(),
    }
}

#[tauri::command]
pub async fn check_update(app: tauri::AppHandle) -> Result<Option<Available>, String> {
    let state = app.state::<Updates>();
    let mut pending = state
        .pending
        .try_lock()
        .map_err(|_| "An update operation is already in progress")?;
    if pending.installed {
        return Err("Restart Harbor to finish the installed update".into());
    }
    // Drop stale metadata before a new check, including checks that fail offline.
    pending.update = None;
    let update = app
        .updater_builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| format!("Could not check GitHub for updates: {e}"))?;
    let available = update.as_ref().map(|u| Available {
        version: u.version.clone(),
        notes: u.body.clone().unwrap_or_default(),
    });
    pending.update = update;
    Ok(available)
}

#[tauri::command]
pub async fn install_update(
    app: tauri::AppHandle,
    version: String,
    on_progress: Channel<Progress>,
) -> Result<(), String> {
    if !can_install() {
        return Err(
            "Install a release build of Harbor in Applications or ~/Applications first".into(),
        );
    }
    let state = app.state::<Updates>();
    // Hold ownership through download/verification/install, rejecting duplicate requests.
    let mut pending = state
        .pending
        .try_lock()
        .map_err(|_| "An update operation is already in progress")?;
    if pending.installed {
        return Err("The update is installed. Restart Harbor to finish".into());
    }
    let mut update = pending
        .update
        .clone()
        .ok_or("Check for updates before installing")?;
    if update.version != version {
        return Err("The selected update changed. Check for updates again".into());
    }
    update.timeout = Some(Duration::from_secs(300));
    let mut downloaded = 0;
    let bytes = update
        .download(
            |chunk, total| {
                downloaded += chunk as u64;
                let _ = on_progress.send(Progress {
                    stage: "downloading",
                    downloaded,
                    total,
                });
            },
            || {
                let _ = on_progress.send(Progress {
                    stage: "verifying",
                    downloaded: 0,
                    total: None,
                });
            },
        )
        .await
        .map_err(|e| format!("Download or signature verification failed: {e}"))?;
    // download() verifies the signature before any files are replaced.
    let _ = on_progress.send(Progress {
        stage: "installing",
        downloaded: 0,
        total: None,
    });
    // Do not let a quit request interrupt replacement of the app bundle.
    state.installing.store(true, Ordering::SeqCst);
    let result = tauri::async_runtime::spawn_blocking(move || update.install(bytes))
        .await
        .map_err(|e| e.to_string());
    state.installing.store(false, Ordering::SeqCst);
    result?.map_err(|e| {
        format!("Could not install the update: {e}. You can also use the GitHub DMG.")
    })?;
    pending.installed = true;
    pending.update = None;
    Ok(())
}

#[tauri::command]
pub async fn restart_after_update(app: tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<Updates>();
    let pending = state
        .pending
        .try_lock()
        .map_err(|_| "An update operation is still in progress")?;
    if !pending.installed {
        return Err("No installed update is waiting for restart".into());
    }
    drop(pending);
    app.restart();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn updates_only_replace_installed_app_bundles() {
        let home = Path::new("/Users/test");
        for path in [
            "/Applications/Harbor.app/Contents/MacOS/harbor-app",
            "/Users/test/Applications/Harbor.app/Contents/MacOS/harbor-app",
        ] {
            assert!(installed_path(Path::new(path), home));
        }
        for path in [
            "/Volumes/Harbor/Harbor.app/Contents/MacOS/harbor-app",
            "/private/var/folders/test/AppTranslocation/Harbor.app/Contents/MacOS/harbor-app",
            "/Users/test/project/target/release/harbor-app",
            "/Applications-other/Harbor.app/Contents/MacOS/harbor-app",
            "/Applications/harbor-app",
            "/Users/other/Applications/Harbor.app/Contents/MacOS/harbor-app",
        ] {
            assert!(!installed_path(Path::new(path), home), "{path}");
        }
    }
}

#[cfg(all(test, target_os = "macos"))]
mod integration {
    use base64::{Engine, engine::general_purpose::STANDARD};
    use serde_json::{Value, json};
    use std::{
        io::{Read, Write},
        net::TcpListener,
        path::Path,
        thread,
        time::Duration,
    };
    use tauri_plugin_updater::UpdaterExt;

    fn fixture() -> Value {
        serde_json::from_str(include_str!("fixtures/updater.json")).unwrap()
    }

    // Loopback HTTP is enabled only in this isolated mock runtime, never in Harbor.
    fn app() -> tauri::App<tauri::test::MockRuntime> {
        let mut context = tauri::test::mock_context(tauri::test::noop_assets());
        context.config_mut().plugins.0.insert(
            "updater".into(),
            json!({
                "pubkey": fixture()["pubkey"],
                "dangerousInsecureTransportProtocol": true
            }),
        );
        tauri::test::mock_builder()
            .plugin(tauri_plugin_updater::Builder::new().build())
            .build(context)
            .unwrap()
    }

    fn server(bytes: Vec<u8>, version: &str, requests: usize) -> (String, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let platform =
            json!({"url": format!("{base}/app.tar.gz"), "signature": fixture()["signature"]});
        let manifest = serde_json::to_vec(&json!({
            "version": version,
            "platforms": {"darwin-aarch64": platform, "darwin-x86_64": platform}
        }))
        .unwrap();
        let thread = thread::spawn(move || {
            for _ in 0..requests {
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut request = [0; 4096];
                let count = socket.read(&mut request).unwrap();
                let body = if request[..count].starts_with(b"GET /latest.json ") {
                    &manifest
                } else {
                    &bytes
                };
                write!(
                    socket,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .unwrap();
                socket.write_all(body).unwrap();
            }
        });
        (format!("{base}/latest.json"), thread)
    }

    fn updater(
        app: &tauri::App<tauri::test::MockRuntime>,
        endpoint: String,
        exe: &Path,
    ) -> tauri_plugin_updater::Updater {
        app.updater_builder()
            .executable_path(exe)
            .endpoints(vec![endpoint.parse().unwrap()])
            .unwrap()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap()
    }

    #[tokio::test]
    async fn signed_download_installs_into_only_the_fixture_bundle() {
        let app = app();
        let folder = tempfile::tempdir().unwrap();
        let exe = folder.path().join("Harbor.app/Contents/MacOS/harbor-app");
        std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
        std::fs::write(&exe, "old version").unwrap();
        let bytes = STANDARD
            .decode(fixture()["archive"].as_str().unwrap())
            .unwrap();
        let (endpoint, worker) = server(bytes, "9.0.0", 2);
        let update = updater(&app, endpoint, &exe)
            .check()
            .await
            .unwrap()
            .unwrap();
        let verified = update.download(|_, _| {}, || {}).await.unwrap();
        update.install(verified).unwrap();
        worker.join().unwrap();
        assert_eq!(
            std::fs::read_to_string(exe).unwrap(),
            "updater integration fixture\n"
        );
    }

    #[tokio::test]
    async fn tampered_download_is_rejected_before_any_replacement() {
        let app = app();
        let folder = tempfile::tempdir().unwrap();
        let exe = folder.path().join("Harbor.app/Contents/MacOS/harbor-app");
        std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
        std::fs::write(&exe, "old version").unwrap();
        let mut bytes = STANDARD
            .decode(fixture()["archive"].as_str().unwrap())
            .unwrap();
        bytes.push(1);
        let (endpoint, worker) = server(bytes, "9.0.0", 2);
        let update = updater(&app, endpoint, &exe)
            .check()
            .await
            .unwrap()
            .unwrap();
        assert!(update.download(|_, _| {}, || {}).await.is_err());
        worker.join().unwrap();
        assert_eq!(std::fs::read_to_string(exe).unwrap(), "old version");
    }

    #[tokio::test]
    async fn older_release_is_not_offered() {
        let app = app();
        let (endpoint, worker) = server(vec![], "0.0.1", 1);
        let exe = Path::new("/Applications/Harbor.app/Contents/MacOS/harbor-app");
        assert!(
            updater(&app, endpoint, exe)
                .check()
                .await
                .unwrap()
                .is_none()
        );
        worker.join().unwrap();
    }
}
