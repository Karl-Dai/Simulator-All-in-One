"""Portable payload configuration must not depend on the Windows ANSI locale."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch


SPEC = importlib.util.spec_from_file_location("suite_windows_utf8", Path(__file__).resolve().parents[1] / "suite.py")
suite = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(suite)


class WindowsUtf8Tests(unittest.TestCase):
    def test_collects_unicode_config_and_resources_under_cp1252_defaults(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "source"
            project = source / "crates/example-app"
            project.mkdir(parents=True)
            binary = source / "target/x86_64-pc-windows-msvc/release/example-app.exe"
            binary.parent.mkdir(parents=True)
            binary.write_bytes(b"fixture executable")
            resource = project / "data/预设.txt"
            resource.parent.mkdir()
            resource.write_text("数据与预设", encoding="utf-8")
            (project / "tauri.conf.json").write_text(json.dumps({
                "description": "数据模拟器",
                "bundle": {"resources": ["data/预设.txt"]},
            }, ensure_ascii=False), encoding="utf-8")
            component = {
                "id": "iec104.master", "repository": "iec104",
                "project": "crates/example-app", "binary": "example-app",
            }
            original_open = Path.open

            def windows_default_open(path, mode="r", buffering=-1, encoding=None,
                                     errors=None, newline=None):
                if "b" not in mode and encoding in (None, "locale"):
                    encoding = "cp1252"
                return original_open(path, mode, buffering, encoding, errors, newline)

            destination = root / "payload"
            with patch.object(Path, "open", windows_default_open):
                copied = suite.copy_portable(source, component, "windows", destination, "amd64")

            self.assertEqual(copied.read_bytes(), binary.read_bytes())
            self.assertEqual((destination / "data/预设.txt").read_bytes(), resource.read_bytes())


if __name__ == "__main__":
    unittest.main()
