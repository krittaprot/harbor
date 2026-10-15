use crate::{config, model::Snapshot};
use std::collections::BTreeMap;
use tauri::{
    Emitter, Manager,
    image::Image,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_notification::NotificationExt;

pub fn installed_bundle() -> bool {
    std::env::current_exe().ok().is_some_and(|p| {
        let text = p.to_string_lossy();
        text.contains(".app/Contents/MacOS/") && !text.contains("/target/")
    })
}

pub fn install(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let open = MenuItem::with_id(app, "open", "Open Harbor", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Harbor", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &quit])?;
    tauri::WebviewWindowBuilder::new(
        app,
        "popover",
        tauri::WebviewUrl::App("popover.html?popover".into()),
    )
    .title("Harbor menu")
    .inner_size(360.0, 600.0)
    .decorations(false)
    .resizable(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .visible(false)
    .build()?;
    TrayIconBuilder::with_id("harbor")
        .icon(Image::from_bytes(include_bytes!(
            "../icons/tray-template.png"
        ))?)
        .icon_as_template(true)
        .tooltip("Harbor · scanning…")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "quit" => app.exit(0),
            "open" => {
                let _ = crate::harbor_commands::show_main(app.clone(), None);
            }
            _ => (),
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                position,
                rect,
                ..
            } = event
                && let Some(window) = tray.app_handle().get_webview_window("popover")
            {
                if window.is_visible().unwrap_or(false) {
                    let _ = window.hide();
                    return;
                }
                let scale = window.scale_factor().unwrap_or(1.0);
                let point = position.to_logical::<f64>(scale);
                let top = rect.position.to_logical::<f64>(scale).y
                    + rect.size.to_logical::<f64>(scale).height
                    + 6.0;
                let mut left = point.x - 180.0;
                if let Ok(Some(monitor)) = window.current_monitor() {
                    let origin = monitor.position().to_logical::<f64>(scale);
                    let size = monitor.size().to_logical::<f64>(scale);
                    left = left.clamp(
                        origin.x + 8.0,
                        (origin.x + size.width - 368.0).max(origin.x + 8.0),
                    );
                }
                let _ = window.set_position(tauri::LogicalPosition::new(left, top));
                let _ = window.show();
                let _ = window.set_focus();
            }
        })
        .build(app)?;
    if installed_bundle() {
        crate::startup::migrate_legacy()?;
        if config::load()?.start_at_login {
            app.autolaunch().enable()?;
        }
    }
    Ok(())
}

pub fn update(app: &tauri::AppHandle, snapshot: &Snapshot) {
    if let Some(tray) = app.tray_by_id("harbor") {
        // Keep the native template icon: macOS renders it white on dark menu bars
        // and adapts its contrast on light ones. Status belongs in the tooltip/popover.
        let _ = tray.set_tooltip(Some(format!(
            "Harbor · {} running · {} need attention",
            snapshot.counts.running, snapshot.counts.attention
        )));
    }
}

#[derive(Default)]
pub struct Notifications {
    previous: BTreeMap<String, (String, Option<String>, u8)>,
}
impl Notifications {
    pub fn update(&mut self, app: &tauri::AppHandle, snapshot: &Snapshot) {
        let enabled =
            config::load().is_ok_and(|c| c.notifications) && !cfg!(feature = "smoke-test");
        for s in snapshot.services.iter().filter(|s| !s.apple) {
            let stamp = s.health.checked_at.map(|t| t.to_rfc3339());
            let previous = self.previous.get(&s.id);
            let failures = if s.health.status == "Failed" {
                previous.map_or(1, |(_, time, count)| {
                    if time == &stamp {
                        *count
                    } else {
                        count.saturating_add(1)
                    }
                })
            } else {
                0
            };
            let stopped = previous.is_some_and(|(state, _, _)| {
                ["Running", "Restarted"].contains(&state.as_str())
                    && ["Failed", "Stopped"].contains(&s.state.as_str())
            });
            let health_failed = failures == 2 && previous.is_some_and(|(_, _, n)| *n < 2);
            if enabled && (stopped || health_failed) {
                let _ = app
                    .notification()
                    .builder()
                    .title(format!("Harbor · {} needs attention", s.name))
                    .body(if stopped {
                        format!("Service is {}", s.state.to_lowercase())
                    } else {
                        "Two consecutive health checks failed".into()
                    })
                    .show();
            }
            self.previous
                .insert(s.id.clone(), (s.state.clone(), stamp, failures));
        }
        self.previous
            .retain(|id, _| snapshot.services.iter().any(|s| &s.id == id));
    }
}

#[tauri::command]
pub fn set_settings(
    app: tauri::AppHandle,
    notifications: bool,
    start_at_login: bool,
) -> Result<(), String> {
    if installed_bundle() {
        crate::startup::migrate_legacy()?;
        if start_at_login {
            app.autolaunch().enable()
        } else {
            app.autolaunch().disable()
        }
        .map_err(|e| e.to_string())?;
    }
    config::update(|c| {
        c.notifications = notifications;
        c.start_at_login = start_at_login;
    })?;
    app.emit(
        "harbor://settings",
        serde_json::json!({ "notifications": notifications, "start_at_login": start_at_login }),
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn quit_harbor(app: tauri::AppHandle) {
    app.exit(0);
}
