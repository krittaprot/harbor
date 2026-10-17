"""Validate both signed updater targets before publishing; write download checksums."""
import hashlib
import json
import sys
from pathlib import Path


def verify(folder: Path, tag: str, notes: str | None = None) -> None:
    manifest = json.loads((folder / "latest.json").read_text())
    if manifest["version"].removeprefix("v") != tag.removeprefix("v"):
        raise ValueError("Updater manifest version differs from the release tag")
    prefix = f"https://github.com/krittaprot/harbor/releases/download/{tag}/"
    latest = "https://github.com/krittaprot/harbor/releases/latest/download/"
    required = ["latest.json"]
    for target, arch in [("darwin-aarch64", "aarch64"), ("darwin-x86_64", "x64")]:
        archive = f"Harbor_{arch}.app.tar.gz"
        signature = f"{archive}.sig"
        dmg = f"Harbor_{tag.removeprefix('v')}_{arch}.dmg"
        expected = (folder / signature).read_text().strip()
        # tauri-action emits floating /latest URLs when uploading via releaseId.
        # Validate its output, then pin both legacy and bundle-specific targets.
        for key in [target, f"{target}-app"]:
            if key.endswith("-app") and key not in manifest["platforms"]:
                continue
            platform = manifest["platforms"][key]
            if platform["url"] not in [prefix + archive, latest + archive]:
                raise ValueError(f"Unexpected download URL for {key}")
            if not expected or platform["signature"].strip() != expected:
                raise ValueError(f"Missing or mismatched signature for {key}")
            platform["url"] = prefix + archive
        for name in [archive, signature, dmg]:
            if (folder / name).stat().st_size == 0:
                raise ValueError(f"Empty release asset: {name}")
        required.extend([archive, signature, dmg])
    if notes is not None:
        manifest["notes"] = notes
    (folder / "latest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    checksums = []
    for name in sorted(required):
        with (folder / name).open("rb") as stream:
            checksums.append(f"{hashlib.file_digest(stream, 'sha256').hexdigest()}  {name}\n")
    (folder / "SHA256SUMS").write_text("".join(checksums))


if __name__ == "__main__":
    notes = json.loads(Path(sys.argv[3]).read_text())["body"] if len(sys.argv) > 3 else None
    verify(Path(sys.argv[1]), sys.argv[2], notes)
    print("Both updater targets, signatures, asset URLs, and release versions match.")
