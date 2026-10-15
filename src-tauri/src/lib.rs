mod commands;
mod error;
mod launchctl;
mod model;
mod plist_util;
mod ports;
mod procs;
mod scan_command;
mod scanner;
#[cfg(feature = "smoke-test")]
mod smoke;
mod types;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            app.manage(scanner::Scanner::start(app.handle().clone()));
            #[cfg(feature = "smoke-test")]
            smoke::install(app);
            Ok(())
        })
        .on_page_load(|webview, payload| {
            #[cfg(feature = "smoke-test")]
            if payload.event() == tauri::webview::PageLoadEvent::Finished {
                webview
                    .eval(include_str!("smoke.js"))
                    .expect("smoke script");
            }
            let _ = (webview, payload);
        })
        .invoke_handler(tauri::generate_handler![
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
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                app.state::<scanner::Scanner>().stop();
            }
        });
}
