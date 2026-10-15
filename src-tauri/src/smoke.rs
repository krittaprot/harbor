//! Opt-in native WebKit/IPC acceptance harness. Not included in normal builds.
use std::sync::atomic::{AtomicI32, Ordering};
use tauri::{Listener, Manager};
static EXIT_CODE: AtomicI32 = AtomicI32::new(2);

pub fn finish() -> ! {
    std::process::exit(EXIT_CODE.load(Ordering::SeqCst));
}

// The foreground test runner itself descends from OpenChamber's LaunchAgent.
// Isolate only its explicitly selected fixture in the smoke inventory so the
// real adoption command can be tested without touching an owner's process.
pub fn isolate_fixture(
    mut processes: Vec<crate::model::ProcInfo>,
    uid: u32,
) -> Vec<crate::model::ProcInfo> {
    let pid = std::env::var("HARBOR_ADOPT_SMOKE_PID")
        .ok()
        .and_then(|v| v.parse::<u32>().ok());
    let expected = Some(include_str!("../../scripts/adopt-fixture.py").to_string());
    let identity = processes
        .iter()
        .find(|p| {
            Some(p.pid) == pid
                && p.uid == Some(uid)
                && expected.as_ref().is_some_and(|e| p.argv.contains(e))
        })
        .map(|p| (p.argv.clone(), p.ppid));
    if let Some((argv, parent)) = identity {
        for p in &mut processes {
            if p.uid == Some(uid) && p.argv == argv && p.ppid == parent {
                p.launchd_label = None;
            }
        }
    }
    processes
}

pub fn install(app: &tauri::App) {
    let popover_app = app.handle().clone();
    app.listen("harbor://smoke-popover", move |_| {
        if let Some(window) = popover_app.get_webview_window("popover") {
            let _ = window.eval(r#"(() => {
              const css = [...document.styleSheets].map(s => [...s.cssRules].map(r => r.cssText).join('\n')).join('\n');
              const html = `<!doctype html><html class="${document.documentElement.className}"><head><style>${css}</style></head><body>${document.body.innerHTML}</body></html>`;
              window.__TAURI_INTERNALS__.invoke('plugin:event|emit', {event:'harbor://smoke-dom',payload:{stage:'popover',html}});
            })()"#);
        }
    });
    app.listen("harbor://smoke-dom", |event| {
        if let Ok(data) = serde_json::from_str::<serde_json::Value>(event.payload())
            && let (Some(stage), Some(html)) = (data["stage"].as_str(), data["html"].as_str())
            && ["light", "dark", "restart", "adopt", "login", "popover"].contains(&stage)
        {
            let dir = crate::config::directory().join("diagnostics");
            if std::fs::create_dir_all(&dir).is_ok() {
                let _ = std::fs::write(dir.join(format!("native-{stage}.html")), html);
            }
        }
    });
    app.listen("harbor://smoke-stage", |event| {
        if let Ok(stage) = serde_json::from_str::<String>(event.payload()) {
            println!("HARBOR_VISUAL_READY {stage}");
        }
    });
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
        let code = if result["ok"] == true && title == "Harbor" && missing.is_empty() {
            0
        } else {
            1
        };
        EXIT_CODE.store(code, Ordering::SeqCst);
        handle.exit(code);
    });
    // Exit through the normal lifecycle even if frontend IPC never connects.
    let handle = app.handle().clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(90));
        eprintln!("Harbor native smoke deadline exceeded");
        handle.exit(2);
    });
}
