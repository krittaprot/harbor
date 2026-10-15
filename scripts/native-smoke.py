"""Run native acceptance in the foreground, capturing only its own window."""
from pathlib import Path
import subprocess
import sys

root = Path(__file__).resolve().parent.parent
output = root / "docs" / "verification"
output.mkdir(parents=True, exist_ok=True)
helper = root / "src-tauri" / "target" / "window-id"
subprocess.run(["swiftc", str(root / "scripts" / "window-id.swift"), "-o", str(helper)], check=True)
process = subprocess.Popen([str(root / "src-tauri" / "target" / "debug" / "harbor-app")], stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, cwd=root)
try:
    for line in process.stdout:
        print(line, end="", flush=True)
        if line.startswith("HARBOR_VISUAL_READY "):
            theme = line.strip().split()[-1]
            if theme not in ("light", "dark", "restart", "adopt", "login", "popover"):
                continue
            result = subprocess.run([str(helper), str(process.pid)], capture_output=True, text=True)
            if result.returncode == 0:
                capture = subprocess.run(["/usr/sbin/screencapture", "-x", "-o", "-l", result.stdout.strip(), str(output / f"harbor-{theme}.png")], capture_output=True, text=True)
                if capture.returncode: print(f"Window capture unavailable: {capture.stderr.strip()}")
            else:
                print(result.stderr.strip())
    sys.exit(process.wait())
except KeyboardInterrupt:
    # Direct unreaped child, created above; identity cannot be reused.
    process.terminate()
    process.wait()
    raise
