//! Process-level tests: no GUI, live inventory or real service mutations needed.
use serde_json::Value;
use std::process::Command;

fn run(args: &[&str], exit: i32) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_harbor"))
        .args(args)
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(exit),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "JSON mode must not mix diagnostics into stderr: {:?}",
        output.stderr
    );
    let value: Value = serde_json::from_slice(&output.stdout).expect("exactly one JSON object");
    assert_eq!(value["schema_version"], 1);
    value
}

#[test]
fn capabilities_are_discoverable_without_opening_the_app() {
    let value = run(&["capabilities", "--json"], 0);
    assert_eq!(value["ok"], true);
    assert_eq!(value["data"]["gui_required"], false);
    assert!(
        value["data"]["commands"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["name"] == "restart")
    );
}

#[test]
fn captured_output_is_json_without_flags_including_errors() {
    assert_eq!(run(&["capabilities"], 0)["ok"], true);
    assert_eq!(run(&["unknown-command"], 2)["error"]["code"], "usage");
    let output = Command::new(env!("CARGO_BIN_EXE_harbor"))
        .args(["restart", "unused", "--text"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).starts_with("harbor: confirmation_required:"));
}

#[test]
fn invalid_requests_have_structured_nonzero_results() {
    for args in [
        vec!["--json", "unknown-command"],
        vec!["ports", "--port", "65536", "--json"],
        vec!["--json", "logs", "label", "--lines", "0"],
    ] {
        let value = run(&args, 2);
        assert_eq!(value["ok"], false);
        assert_eq!(value["error"]["code"], "usage");
    }
}

#[test]
fn mutations_never_implicitly_confirm_or_prompt() {
    let value = run(&["restart", "never-touch-a-real-service", "--json"], 2);
    assert_eq!(value["error"]["code"], "confirmation_required");
    let value = run(&["restart", "unused", "--yes", "--dry-run", "--json"], 2);
    assert_eq!(value["error"]["code"], "usage");
    let value = run(&["adopt", "unused", "as", "com.harbor.never-create"], 2);
    assert_eq!(value["error"]["code"], "confirmation_required");
    let value = run(&["archive", "com.harbor.never-archive"], 2);
    assert_eq!(value["error"]["code"], "confirmation_required");
    let value = run(
        &[
            "preview",
            "adopt",
            "unused",
            "wrong-word",
            "com.harbor.test",
        ],
        2,
    );
    assert_eq!(value["error"]["code"], "usage");
}
