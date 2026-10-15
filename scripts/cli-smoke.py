"""Foreground CLI integration against a uniquely named, owned throwaway LaunchAgent.

Run after `cargo build --manifest-path src-tauri/Cargo.toml --bin harbor`.
Pass a binary path to test a release or installed CLI instead.
"""
from pathlib import Path
import json
import os
import plistlib
import shutil
import subprocess
import sys
import time

root = Path(__file__).resolve().parent.parent
binary = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else root / "src-tauri/target/debug/harbor"
label = f"com.harbor.cli-smoke-{os.getpid()}"
path = Path.home() / f"Library/LaunchAgents/{label}.plist"
logs = Path.home() / f"Library/Logs/{label}"
fixture_code = (root / "scripts/adopt-fixture.py").read_text()
config = {
    "Label": label,
    "ProgramArguments": [sys.executable, "-u", "-c", fixture_code],
    "WorkingDirectory": str(Path.home()),
    "RunAtLoad": True,
    "KeepAlive": False,
    "StandardOutPath": str(logs / "out.log"),
    "StandardErrorPath": str(logs / "err.log"),
}

def cli(*args, exit=0):
    result = subprocess.run([str(binary), *args, "--json"], capture_output=True, text=True, timeout=45)
    value = json.loads(result.stdout)
    assert result.returncode == exit, (args, result.returncode, value)
    assert not result.stderr, result.stderr
    assert value["schema_version"] == 1
    return value

def loaded():
    return label in subprocess.check_output(["/bin/launchctl", "list"], text=True)

def ready(previous=None):
    deadline = time.monotonic() + 20
    while time.monotonic() < deadline:
        service = cli("show", label)["data"]
        pid = service["jobs"][0]["pid"]
        if pid and pid != previous and service["ports"]:
            return service
        time.sleep(.2)
    raise AssertionError("Fixture did not obtain a listener")

assert not path.exists() and not logs.exists() and not loaded()
logs.mkdir(parents=True)
with path.open("xb") as file:
    plistlib.dump(config, file)
success = False
try:
    cli("capabilities")
    result = cli("restart", label, exit=2)
    assert result["error"]["code"] == "confirmation_required" and not loaded()
    result = cli("start", label, "--dry-run")
    assert result["data"]["dry_run"] and not loaded()
    assert [job["label"] for job in result["data"]["plan"]["jobs"]] == [label]
    cli("start", label, "--yes")
    first = ready()
    first_pid = first["jobs"][0]["pid"]
    port = first["ports"][0]["port"]
    assert any(p["pid"] == first_pid for p in cli("ports", "--port", str(port))["data"])
    assert cli("health", label)["data"]["services"][0]["health"]["status"] == "Ok"
    assert str(port) in cli("logs", label, "--stream", "stdout")["data"]["text"]
    cli("restart", label, "--yes")
    second = ready(first_pid)
    second_pid = second["jobs"][0]["pid"]
    cli("stop", label, "--yes")
    assert not loaded()
    assert cli("show", label)["data"]["jobs"][0]["pid"] is None
    for pid in [first_pid, second_pid]:
        assert subprocess.run(["/bin/ps", "-p", str(pid)], stdout=subprocess.DEVNULL).returncode != 0
    assert cli("show", "harbor-does-not-exist-fixture", exit=3)["error"]["code"] == "not_found"
    success = True
    print("CLI smoke passed: confirmation, dry-run, start, status, ports, health, logs, restart, stop and not-found.")
finally:
    assert plistlib.loads(path.read_bytes()) == config, "Fixture identity changed; refusing cleanup"
    if loaded():
        subprocess.run(["/bin/launchctl", "bootout", f"gui/{os.getuid()}", str(path)], check=True)
    assert not loaded(), "Fixture still loaded"
    path.unlink()
    if success:
        shutil.rmtree(logs)
    else:
        print(f"Fixture logs retained: {logs}")
    print("Owned fixture LaunchAgent and plist removed.")
