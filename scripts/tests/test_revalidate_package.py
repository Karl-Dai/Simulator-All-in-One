import importlib.util
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
SPEC = importlib.util.spec_from_file_location("revalidate", Path(__file__).resolve().parents[1] / "revalidate_package.py")
reuse = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(reuse)


class PackageReuseTests(unittest.TestCase):
    def test_reuse_rejects_changed_compiled_inputs_and_untrusted_runs(self):
        metadata = {"repository": {"full_name": "owner/suite"},
                    "head_repository": {"full_name": "owner/suite"},
                    "head_branch": "main", "event": "workflow_dispatch",
                    "path": ".github/workflows/build-suite.yml", "status": "completed",
                    "head_sha": "a" * 40}
        reuse.validate_origin(metadata, "owner/suite", ["packaging/linux/validation.Dockerfile", "scripts/smoke.py"], "linux")
        reuse.validate_origin(metadata, "owner/suite", ["packaging/windows/check-payload.ps1"], "windows")
        for path in ("main.go", "patches/pmu.patch", "components.lock.json", "frontend/src/App.vue", "scripts/suite.py",
                     "toolchain.lock.json", "scripts/toolchain.py"):
            with self.subTest(path=path), self.assertRaises(reuse.suite.SuiteError):
                reuse.validate_origin(metadata, "owner/suite", [path], "linux")
        for path in ("packaging/linux/runtime.py", "packaging/linux/run-simlab.sh", "packaging/linux/self-extract.sh"):
            with self.subTest(path=path), self.assertRaises(reuse.suite.SuiteError):
                reuse.validate_origin(metadata, "owner/suite", [path], "linux")
        for key, value in (("event", "pull_request"), ("head_branch", "untrusted"), ("head_repository", {"full_name": "fork/suite"})):
            with self.subTest(key=key), self.assertRaises(reuse.suite.SuiteError):
                reuse.validate_origin(dict(metadata, **{key: value}), "owner/suite", [], "linux")

    def test_only_declared_payload_files_are_repacked(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            original = root / "extracted"
            (original / "apps/role").mkdir(parents=True)
            for name in ("suite-manifest.json", "SAIO.exe", "Uninstall.exe", "apps/role/child.exe"):
                (original / name).write_bytes(name.encode())
            manifest = {"launcherFiles": {"SAIO.exe": {"type": "file"}},
                        "components": [{"payloadRoot": "apps/role", "payloadFiles": {"child.exe": {"type": "file"}}}]}
            destination = root / "stage"
            reuse.copy_declared_windows_payload(original, destination, manifest)
            self.assertEqual(sorted(str(p.relative_to(destination)) for p in destination.rglob("*") if p.is_file()),
                             sorted(str(Path(p)) for p in ("suite-manifest.json", "SAIO.exe", "apps/role/child.exe")))
            self.assertEqual((destination / "apps/role/child.exe").read_bytes(), b"apps/role/child.exe")
            self.assertFalse((destination / "Uninstall.exe").exists())


if __name__ == "__main__":
    unittest.main()
