import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location("sync_ui", Path(__file__).resolve().parents[1] / "sync_ui.py")
sync_ui = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(sync_ui)


class SharedUIExportTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.workspace = self.root / "components"
        self.destination = self.workspace / "modbus/frontend/src/simlab-ui"
        (self.root / "ui/contracts").mkdir(parents=True)
        (self.root / "ui/theme.ts").write_text("export const theme = 'system'\n")
        (self.root / "ui/contracts/sample.json").write_text("{}")
        (self.root / "ui/preferences.go").write_text("package suitehost\n")
        (self.root / "patches").mkdir()
        self.destination.mkdir(parents=True)
        (self.root / "components.lock.json").write_text(json.dumps({
            "repositories": [{"id": "modbus", "directory": "modbus", "commit": "a" * 40, "patch": "patches/modbus.patch"}],
            "components": [{"id": "modbus.master", "repository": "modbus", "frontend": "frontend"}],
        }))

    def export(self):
        def output(args, **kwargs):
            return "a" * 40 if "rev-parse" in args else b"reviewed patch"
        with patch.object(sync_ui.subprocess, "check_output", side_effect=output), patch.object(sync_ui.subprocess, "run"):
            sync_ui.write(self.workspace, self.root)

    def test_recursive_assets_provenance_and_drift_detection(self):
        self.export()
        sync_ui.check(self.workspace, self.root)
        first = (self.destination / ".source.json").read_bytes()
        self.export()
        self.assertEqual(first, (self.destination / ".source.json").read_bytes())
        self.assertTrue((self.destination / "contracts/sample.json").is_file())
        (self.destination / "theme.ts").write_text("local change")
        with self.assertRaisesRegex(ValueError, "Stale shared UI asset"):
            sync_ui.check(self.workspace, self.root)

    def add_old_record(self, name, contents):
        (self.destination / ".source.json").write_text(json.dumps({"files": {
            name: hashlib.sha256(contents.encode()).hexdigest(),
        }}))

    def test_removes_only_unchanged_declared_old_assets(self):
        (self.destination / "old.css").write_text("old")
        (self.destination / "unrelated.css").write_text("user")
        self.add_old_record("old.css", "old")
        self.export()
        self.assertFalse((self.destination / "old.css").exists())
        self.assertEqual((self.destination / "unrelated.css").read_text(), "user")

    def test_preserves_modified_old_asset(self):
        (self.destination / "old.css").write_text("user change")
        self.add_old_record("old.css", "old")
        with self.assertRaisesRegex(ValueError, "modified generated asset"):
            self.export()
        self.assertEqual((self.destination / "old.css").read_text(), "user change")

    def test_rejects_escaping_record_without_deleting_file(self):
        outside = self.destination.parent / "outside.css"
        outside.write_text("user")
        self.add_old_record("../outside.css", "user")
        with self.assertRaisesRegex(ValueError, "Unsafe generated asset path"):
            self.export()
        self.assertEqual(outside.read_text(), "user")


if __name__ == "__main__":
    unittest.main()
