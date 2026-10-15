"""Foreground adoption/diagnosis/archival checks using only owned fixtures.

Build first: cargo build --manifest-path src-tauri/Cargo.toml --features smoke-test --bin harbor
The smoke feature exposes only this harness's verified fixture processes as unmanaged.
"""
from pathlib import Path
import json
import os
import plistlib
import shutil
import subprocess
import sys

root = Path(__file__).resolve().parent.parent
binary = root / "src-tauri/target/debug/harbor"
code = (root / "scripts/adopt-fixture.py").read_text()
children = [subprocess.Popen([sys.executable, "-c", code], stdout=subprocess.PIPE, text=True, cwd=Path.home()) for _ in range(2)]
env = {**os.environ, "HARBOR_ADOPT_SMOKE_PID": str(children[0].pid)}
labels = [f"com.harbor.cli-adopt-{children[0].pid}", f"com.harbor.cli-failed-{children[0].pid}"]
base = Path.home() / "Library/LaunchAgents"

def cli(*args, exit=0):
    p = subprocess.run([str(binary), *args], env=env, text=True, capture_output=True, timeout=40)
    result = json.loads(p.stdout)
    assert p.returncode == exit and not p.stderr, (args, p.returncode, result, p.stderr)
    return result

try:
    for child in children:
        assert int(child.stdout.readline()) > 0
    for label in labels:
        assert not (base / f"{label}.plist").exists()
        assert not (base / ".harbor-archive" / f"{label}.plist").exists()
    groups = cli("unmanaged")["data"]["unmanaged"]
    group = next(g for g in groups if any(p["pid"] == children[0].pid for p in g["copies"]))
    assert len(group["copies"]) == 2
    key = group["key"]
    preview = cli("preview", "adopt", key, "as", labels[0])["data"]
    assert preview["dry_run"] and "<plist" in preview["xml"]
    assert not (base / f"{labels[0]}.plist").exists()
    failure = cli("adopt", key, "as", labels[1], "--no-login", "--no-keep-alive", "--yes", exit=1)
    assert failure["error"]["code"] == "adoption_failed"
    assert any(e["stage"] == "rollback" for e in failure["error"]["details"]["progress"])
    assert all(c.poll() is None for c in children), "Failure must preserve originals"
    diagnosis = cli("diagnose", labels[1])["data"]
    assert diagnosis["last_adoption"]["error"]
    assert diagnosis["jobs"][0]["logs"]["stdout"]["path"]
    archive = cli("preview", "archive", labels[1])["data"]
    assert archive["dry_run"] and (base / f"{labels[1]}.plist").exists()
    archive = cli("archive", labels[1], "--yes")["data"]
    assert archive["archived"] and Path(archive["destination"]).is_file()
    assert cli("diagnose", labels[1])["data"]["jobs"][0]["plist_path"] == archive["destination"]
    adopted = cli("adopt", key, "as", labels[0], "--stop-originals", "--yes")["data"]
    assert adopted["ready"] and any(e["stage"] == "complete" for e in adopted["progress"])
    assert all(c.wait(timeout=5) == -15 for c in children)
    assert cli("diagnose", labels[0])["data"]["last_adoption"]["results"]
    print("CLI adoption passed: preview, readiness failure/rollback, preserved originals, saved diagnostics, archive/diagnose archived job, successful adoption and SIGTERM cleanup.")
finally:
    for label in labels:
        for path in [base / f"{label}.plist", base / ".harbor-archive" / f"{label}.plist"]:
            if path.exists():
                data = plistlib.loads(path.read_bytes())
                assert data["Label"] == label and code in data["ProgramArguments"]
                subprocess.run(["/bin/launchctl", "bootout", f"gui/{os.getuid()}", str(path)], capture_output=True)
                loaded = subprocess.check_output(["/bin/launchctl", "list"], text=True)
                assert label not in loaded
                path.unlink()
        logs = Path.home() / "Library/Logs" / label
        if logs.exists(): shutil.rmtree(logs)
        record = Path.home() / "Library/Application Support/Harbor/adoptions" / f"{label}.json"
        if record.exists():
            assert json.loads(record.read_text())["label"] == label
            record.unlink()
    for child in children:
        if child.poll() is None:
            child.terminate()
            child.wait(timeout=5)
    print("Owned CLI fixtures, agents, archives, logs and reports removed.")
