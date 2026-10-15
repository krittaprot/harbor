"""Foreground native adoption test; only its own throwaway processes/label are changed."""
from pathlib import Path
import os
import json
import plistlib
import subprocess
import sys

root = Path(__file__).resolve().parent.parent
fixture = root / "scripts/adopt-fixture.py"
fixture_code = fixture.read_text()
children = [subprocess.Popen([sys.executable, "-c", fixture_code], stdout=subprocess.PIPE, text=True, cwd=Path.home()) for _ in range(2)]
label = f"com.harbor.smoke.{children[0].pid}"
plist = Path.home() / f"Library/LaunchAgents/{label}.plist"
assert not plist.exists(), "Refuse to reuse an existing plist"
try:
    ports = [int(child.stdout.readline()) for child in children]
    print(f"Owned fixture PIDs {[c.pid for c in children]}, ports {ports}", flush=True)
    result = subprocess.run([str(root / "src-tauri/target/debug/launchd-ui")], env={**os.environ, "HARBOR_ADOPT_SMOKE_PID": str(children[0].pid)}, cwd=root)
    assert result.returncode == 0, f"Native smoke failed: {result.returncode}"
    for child in children:
        code = child.wait(timeout=5)
        assert code == -15, f"Expected graceful SIGTERM, got {code}"
    print("Adoption smoke: new agent became ready, both original copies exited with SIGTERM", flush=True)
finally:
    if plist.exists():
        data = plistlib.loads(plist.read_bytes())
        assert data.get("Label") == label and fixture_code in data.get("ProgramArguments", []), "Refuse cleanup: unexpected plist identity"
        result = subprocess.run(["/bin/launchctl", "bootout", f"gui/{os.getuid()}", str(plist)], capture_output=True, text=True)
        loaded = subprocess.run(["/bin/launchctl", "list"], capture_output=True, text=True, check=True)
        assert label not in loaded.stdout, "Fixture still loaded"
        plist.unlink()
        log = Path.home() / f"Library/Logs/{label}/err.log"
        if log.exists(): print(log.read_text(), flush=True)
    for child in children:
        if child.poll() is None:
            child.terminate() # Owned unreaped direct child with the argv above.
            child.wait(timeout=5)
    report = Path.home() / f"Library/Application Support/Harbor/adoptions/{label}.json"
    if report.exists():
        assert json.loads(report.read_text())["label"] == label
        report.unlink()
    print("Owned fixture processes and plist cleaned up", flush=True)
