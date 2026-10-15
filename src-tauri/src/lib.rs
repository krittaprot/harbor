mod adopt;
pub mod cli;
mod commands;
mod config;
mod error;
mod grouping;
mod harbor_commands;
mod health;
mod launchctl;
mod login;
mod model;
mod operations;
mod plist_util;
mod ports;
mod procs;
mod scan_command;
mod scanner;
#[cfg(feature = "smoke-test")]
mod smoke;
mod snapshots;
mod system;
mod tray;
mod types;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .macos_launcher(tauri_plugin_autostart::MacosLauncher::LaunchAgent)
                .build(),
        )
        .setup(|app| {
            app.manage(login::LoginCache::default());
            app.manage(snapshots::SnapshotState::default());
            tray::install(app)?;
            app.manage(scanner::Scanner::start(app.handle().clone()));
            #[cfg(feature = "smoke-test")]
            smoke::install(app);
            Ok(())
        })
        .on_page_load(|webview, payload| {
            #[cfg(feature = "smoke-test")]
            if webview.label() == "main"
                && payload.event() == tauri::webview::PageLoadEvent::Finished
            {
                let fixture = std::env::var("HARBOR_ADOPT_SMOKE_PID")
                    .ok()
                    .and_then(|v| v.parse::<u32>().ok())
                    .unwrap_or(0);
                webview
                    .eval(format!("window.__HARBOR_FIXTURE_PID__ = {fixture};"))
                    .expect("fixture selection");
                webview
                    .eval(include_str!("smoke.js"))
                    .expect("smoke script");
            }
            let _ = (webview, payload);
        })
        .invoke_handler(tauri::generate_handler![
            tray::set_settings,
            tray::quit_harbor,
            harbor_commands::rescan_now,
            harbor_commands::service_action,
            harbor_commands::get_settings,
            harbor_commands::ignore_unmanaged,
            harbor_commands::pin_service,
            harbor_commands::archive_plist,
            harbor_commands::open_url,
            harbor_commands::show_main,
            adopt::adopt_preview,
            adopt::adopt_apply,
            adopt::stop_unmanaged,
            login::list_login_items,
            login::set_login_item,
            snapshots::save_snapshot,
            snapshots::last_restart_report,
            snapshots::restart_machine,
            commands::get_snapshot,
            commands::list_jobs,
            commands::get_job_detail,
            commands::start_job,
            commands::stop_job,
            commands::restart_job,
            commands::kickstart_job,
            commands::enable_job,
            commands::disable_job,
            commands::save_job,
            commands::save_raw_plist,
            commands::create_job,
            commands::delete_job,
            commands::read_log_file,
            commands::clear_log_file,
            commands::open_log_in_editor,
            commands::get_home_dir,
            commands::reveal_in_finder,
        ])
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::Focused(false) if window.label() == "popover" => {
                let _ = window.hide();
            }
            tauri::WindowEvent::CloseRequested { api, .. }
                if window.label() == "main" && !cfg!(feature = "smoke-test") =>
            {
                api.prevent_close();
                let _ = window.hide();
            }
            _ => (),
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                app.state::<scanner::Scanner>().stop();
                #[cfg(feature = "smoke-test")]
                smoke::finish();
            }
        });
}
