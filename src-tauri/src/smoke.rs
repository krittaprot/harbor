//! Opt-in native WebKit/IPC acceptance harness. Not included in normal builds.
use tauri::{Listener, Manager};

pub fn install(app: &tauri::App) {
    let handle = app.handle().clone();
    app.listen("harbor://smoke-result", move |event| {
        let mut result: serde_json::Value =
            serde_json::from_str(event.payload()).expect("smoke result");
        let output = crate::scan_command::output(
            "/usr/sbin/lsof",
            &["-nP", "-iTCP", "-sTCP:LISTEN", "-F", "pcn"],
        )
        .expect("independent lsof comparison");
        let ports = crate::ports::parse_lsof(&String::from_utf8_lossy(&output.stdout));
        let observed: Vec<crate::model::PortInfo> =
            serde_json::from_value(result["ports"].take()).unwrap_or_default();
        let missing: Vec<_> = ports.iter().filter(|p| !observed.contains(p)).collect();
        let extra: Vec<_> = observed.iter().filter(|p| !ports.contains(p)).collect();
        result["binding_count"] = observed.len().into();
        result["lsof_binding_count"] = ports.len().into();
        result["missing_from_snapshot"] = serde_json::to_value(&missing).unwrap();
        result["extra_in_snapshot"] = serde_json::to_value(&extra).unwrap();
        result["acceptance_ports_present"] = serde_json::json!(
            [3216, 5432, 9230].map(|port| observed.iter().any(|p| p.port == port))
        );
        println!("HARBOR_NATIVE_SMOKE {result}");
        let title = handle.get_webview_window("main").unwrap().title().unwrap();
        println!("HARBOR_NATIVE_TITLE {title}");
        handle.exit(
            if result["ok"] == true && title == "Harbor" && missing.is_empty() {
                0
            } else {
                1
            },
        );
    });
    // Exit through the normal lifecycle even if frontend IPC never connects.
    let handle = app.handle().clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(40));
        eprintln!("Harbor native smoke deadline exceeded");
        handle.exit(2);
    });
}
