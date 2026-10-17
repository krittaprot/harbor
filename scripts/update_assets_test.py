import hashlib
import json
import runpy
import tempfile
import unittest
from pathlib import Path

verify = runpy.run_path(str(Path(__file__).with_name("verify-update-assets.py")))["verify"]


class UpdateAssetsTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.folder = Path(self.temp.name)
        self.manifest = {"version": "1.5.2", "platforms": {}}
        for target, arch in [("darwin-aarch64", "aarch64"), ("darwin-x86_64", "x64")]:
            archive = f"Harbor_{arch}.app.tar.gz"
            (self.folder / archive).write_bytes(b"fixture archive")
            (self.folder / f"{archive}.sig").write_text(f"fixture-signature-{arch}")
            (self.folder / f"Harbor_1.5.2_{arch}.dmg").write_bytes(b"fixture dmg")
            self.manifest["platforms"][target] = {
                "url": f"https://github.com/krittaprot/harbor/releases/download/v1.5.2/{archive}",
                "signature": f"fixture-signature-{arch}",
            }

    def check(self):
        (self.folder / "latest.json").write_text(json.dumps(self.manifest))
        verify(self.folder, "v1.5.2")

    def test_complete_release_writes_correct_checksums(self):
        self.check()
        checksums = (self.folder / "SHA256SUMS").read_text().splitlines()
        self.assertEqual(len(checksums), 7)
        for line in checksums:
            digest, name = line.split("  ")
            self.assertEqual(digest, hashlib.sha256((self.folder / name).read_bytes()).hexdigest())

    def test_incomplete_or_misdirected_manifest_is_rejected(self):
        original = json.dumps(self.manifest)
        for mutation in [
            lambda m: m.update(version="1.5.1"),
            lambda m: m["platforms"].pop("darwin-x86_64"),
            lambda m: m["platforms"]["darwin-aarch64"].update(signature="wrong"),
            lambda m: m["platforms"]["darwin-aarch64"].update(url="https://example.com/update"),
        ]:
            with self.subTest(mutation=mutation):
                self.manifest = json.loads(original)
                mutation(self.manifest)
                with self.assertRaises((ValueError, KeyError)):
                    self.check()
                self.assertFalse((self.folder / "SHA256SUMS").exists())

    def test_missing_and_empty_assets_are_rejected(self):
        dmg = self.folder / "Harbor_1.5.2_x64.dmg"
        dmg.write_bytes(b"")
        with self.assertRaises(ValueError):
            self.check()
        dmg.unlink()
        with self.assertRaises(FileNotFoundError):
            self.check()


if __name__ == "__main__":
    unittest.main()
