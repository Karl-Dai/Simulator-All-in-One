"""Validate real portable payload inputs without executing a Windows binary."""
import hashlib
import json
from pathlib import Path
import shutil
import sys
import tempfile
import unittest
import zipfile

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import windows_portable

ROOT = Path(__file__).resolve().parents[2]


class WindowsPortableTests(unittest.TestCase):
    def test_embeds_all_payloads_and_renders_native_metadata_without_webview(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            stage = root / "stage"
            stage.mkdir()
            for role in ("launcher", "modbus.master", "modbus.slave", "iec104.master", "iec104.slave", "pmu.master", "pmu.sub", "opcua.master", "opcua.server"):
                (stage / (role + ".exe")).write_bytes(role.encode())
            (stage / "suite-manifest.json").write_text('{"suiteVersion":"0.1.4"}')
            calls = []

            def run(args, cwd=None, env=None):
                calls.append(args)
                if args[0] != "go":
                    return
                with zipfile.ZipFile(cwd / "payload.zip") as archive:
                    self.assertEqual(set(archive.namelist()), {p.name for p in stage.iterdir()})
                    index = json.loads((cwd / "payload-index.json").read_text())
                    for name in archive.namelist():
                        self.assertEqual(index[name]["sha256"], hashlib.sha256(archive.read(name)).hexdigest())
                manifest = (cwd / "portable.manifest").read_text()
                self.assertNotIn("{{", manifest)
                self.assertIn('version="0.1.4.0"', manifest)
                self.assertIn('FILEVERSION 0,1,4,0', (cwd / "portable.rc").read_text())
                self.assertIn("go:embed payload.zip", (cwd / "payload.go").read_text())
                self.assertEqual(env["CGO_ENABLED"], "0")
                self.assertEqual(env["GOOS"], "windows")
                Path(args[-2]).write_bytes(b"MZ fixture")

            destination = root / "portable.exe"
            windows_portable.build(stage, destination, "0.1.4", ROOT, run,
                                   lambda p: hashlib.sha256(p.read_bytes()).hexdigest())
            self.assertTrue(destination.is_file())
            self.assertIn("-s -w -H=windowsgui", calls[-1])

    def test_rejects_embedded_installer_and_fixed_runtime(self):
        for name in ("WebView2Runtime.exe", "Uninstall.exe", "webview2/msedgewebview2.exe"):
            with self.subTest(name=name), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                stage = root / "stage"
                file = stage / name
                file.parent.mkdir(parents=True)
                file.write_bytes(b"must not be embedded")
                with self.assertRaisesRegex(ValueError, "must not contain"):
                    windows_portable.build(stage, root / "portable.exe", "0.1.4", ROOT,
                                           lambda *a, **k: self.fail("compiler must not run"),
                                           lambda p: "0" * 64)
