import importlib.util
import json
import os
from pathlib import Path
import plistlib
import shutil
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location("suite", Path(__file__).resolve().parents[1] / "suite.py")
suite = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(suite)


class SuiteTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.workspace = self.root / ".components"
        self.lock = suite.read_lock()
        for repo in self.lock["repositories"]:
            (self.root / repo["patch"]).parent.mkdir(exist_ok=True)
            (self.root / repo["patch"]).write_text("fixture patch\n")
            if repo.get("cargoLock"):
                target = self.root / repo["cargoLock"]
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_text("fixture dependency lock\n")
                repo["cargoLockSha256"] = suite.digest(target)
        suite.write_json(self.root / "components.lock.json", self.lock)

    def make_app(self, app, identity, version, content=b"fixture"):
        binary = app / "Contents/MacOS/payload"
        binary.parent.mkdir(parents=True)
        binary.write_bytes(content)
        info = {"CFBundleExecutable": "payload", "CFBundleIdentifier": identity,
                "CFBundleShortVersionString": version, "LSMinimumSystemVersion": "13.0"}
        (app / "Contents/Info.plist").write_bytes(plistlib.dumps(info))
        return binary

    def test_windows_build_uses_git_bash_instead_of_system_wsl_shim(self):
        git = self.root / "Git/cmd/git.exe"
        shell = self.root / "Git/bin/bash.exe"
        git.parent.mkdir(parents=True)
        git.touch()
        shell.parent.mkdir(parents=True)
        shell.touch()
        self.assertEqual(Path(suite.git_bash(git)), shell.resolve())
        shell.unlink()
        system_shell = self.root / "System32/bash.exe"
        system_shell.parent.mkdir()
        system_shell.touch()
        with self.assertRaisesRegex(suite.SuiteError, "WSL"):
            suite.git_bash(git)

    def mac_fixture(self):
        self.make_app(self.root / "build/bin/SimLab.app", "cloud.carldai.simlab", "0.1.0")
        repos = {item["id"]: item for item in self.lock["repositories"]}
        for component in self.lock["components"]:
            source = self.workspace / repos[component["repository"]]["directory"]
            parent = source / (component["project"] + "/build/bin" if component["repository"] == "modbus" else "target/release/bundle/macos")
            self.make_app(parent / (component["product"] + ".app"), component["bundleId"], component["version"])

    def test_collect_hashes_after_child_signing_and_before_outer_signing(self):
        self.mac_fixture()
        signed = []

        def sign_child(app, identity):
            entry, _ = suite.app_entry(app)
            entry.write_bytes(entry.read_bytes() + b"signed")
            signed.append(app.name)

        def command(args, **kwargs):
            if args[:3] == ["codesign", "--force", "--sign"]:
                app = Path(args[-1])
                manifest = app / "Contents/Resources/suite-manifest.json"
                self.assertEqual(len(signed), 8)
                self.assertTrue(manifest.is_file())
                suite.verify_manifest(app / "Contents", self.lock)

        with patch.object(suite, "assert_source"), patch.object(suite, "assert_architecture"), \
                patch.object(suite, "sign_app", side_effect=sign_child), patch.object(suite, "run", side_effect=command):
            output = suite.collect(self.workspace, "darwin", "arm64", root=self.root, adopt=True)
        self.assertEqual(len(signed), 8)
        manifest = suite.verify_manifest(output / "SimLab.app/Contents", self.lock)
        self.assertTrue(all(item["sha256"] == __import__("hashlib").sha256(b"fixturesigned").hexdigest() for item in manifest["components"]))
        resource = output / "SimLab.app/Contents/Helpers/ModbusMaster.app/Contents/unrecorded-resource"
        resource.write_text("unrecorded build output")
        with self.assertRaisesRegex(suite.SuiteError, "resource tree changed"):
            suite.verify_manifest(output / "SimLab.app/Contents", self.lock)

    def test_missing_role_leaves_previous_complete_stage_untouched(self):
        self.mac_fixture()
        missing = self.workspace / "OPCUASim/target/release/bundle/macos/OPCUAServer.app"
        __import__("shutil").rmtree(missing)
        output = self.root / "dist/staging/darwin-arm64"
        output.mkdir(parents=True)
        marker = output / "previous-good-package"
        marker.write_text("preserve")
        with patch.object(suite, "assert_source"), patch.object(suite, "assert_architecture"), patch.object(suite, "sign_app"):
            with self.assertRaises(suite.SuiteError):
                suite.collect(self.workspace, "darwin", "arm64", root=self.root, adopt=True)
        self.assertEqual(marker.read_text(), "preserve")

    def test_manifest_rejects_missing_duplicate_tampered_and_escaping_roles(self):
        entries = []
        for role in suite.EXPECTED_IDS:
            binary = self.root / "apps" / role / "payload"
            binary.parent.mkdir(parents=True)
            binary.write_bytes(role.encode())
            entries.append({"id": role, "version": "1.0.0", "sourceCommit": "a" * 40,
                            "executable": binary.relative_to(self.root).as_posix(),
                            "sha256": suite.digest(binary), "suiteBuild": True})
        manifest = {"schemaVersion": 1, "suiteVersion": "0.1.0", "platform": "linux", "architecture": "amd64", "components": entries}
        target = self.root / "suite-manifest.json"
        suite.write_json(target, manifest)
        suite.verify_manifest(self.root)
        for bad_entries in (entries[:-1], entries[:-1] + [entries[0]], [dict(entries[0], sha256="0" * 64)] + entries[1:], [dict(entries[0], executable="../outside")] + entries[1:]):
            with self.subTest(entries=bad_entries):
                suite.write_json(target, dict(manifest, components=bad_entries))
                with self.assertRaises(suite.SuiteError):
                    suite.verify_manifest(self.root)

    def test_bootstrap_rejects_dirty_clone_without_changing_it(self):
        first = self.lock["repositories"][0]
        repo = self.workspace / first["directory"]
        repo.mkdir(parents=True)
        subprocess.run(["git", "init", "-q", repo], check=True)
        file = repo / "source"
        file.write_text("original")
        subprocess.run(["git", "-C", repo, "add", "source"], check=True)
        subprocess.run(["git", "-C", repo, "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "commit", "-qm", "fixture"], check=True)
        file.write_text("user's uncommitted work")
        with self.assertRaisesRegex(suite.SuiteError, "dirty checkout"):
            suite.bootstrap(self.workspace, root=self.root)
        self.assertEqual(file.read_text(), "user's uncommitted work")

    def test_wails_doctype_plist_and_malicious_entry(self):
        app = self.root / "Payload.app"
        self.make_app(app, "fixture", "1.0")
        path = app / "Contents/Info.plist"
        path.write_bytes(path.read_bytes().split(b"\n", 1)[1])
        suite.app_entry(app)
        info = plistlib.loads(path.read_bytes(), fmt=plistlib.FMT_XML)
        info["CFBundleExecutable"] = "../../../outside"
        path.write_bytes(plistlib.dumps(info))
        with self.assertRaises(suite.SuiteError):
            suite.app_entry(app)

    def test_symlink_outside_suite_is_rejected(self):
        try:
            (self.root / "outside-link").symlink_to(self.root.parent)
        except OSError:
            self.skipTest("host does not permit symlinks")
        with self.assertRaises(suite.SuiteError):
            suite.assert_tree_links(self.root)

    @unittest.skipIf(os.name == "nt", "Linux self-extractor is tested on POSIX builders")
    def test_self_extractor_repairs_missing_payload_and_preserves_damaged_directory(self):
        if not shutil.which("sha256sum"):
            self.skipTest("sha256sum is required by the Linux archive format")
        payload = self.root / "payload/SimLab"
        payload.mkdir(parents=True)
        (payload / "program").write_bytes(b"expected program")
        wrapper = payload / "run-simlab.sh"
        wrapper.write_text("#!/bin/sh\nexit 0\n")
        wrapper.chmod(0o755)
        index = payload / ".files.sha256"
        index.write_text("".join(f"{suite.digest(path)}  {path.name}\n" for path in sorted(payload.iterdir()) if path.is_file()))
        archive = self.root / "archive.tar.gz"
        with tarfile.open(archive, "w:gz") as stream:
            stream.add(payload, arcname="SimLab")
        header = (suite.ROOT / "packaging/linux/self-extract.sh").read_text().replace("@PAYLOAD_SHA256@", suite.digest(archive)).replace("@INDEX_SHA256@", suite.digest(index)).replace("@VERSION@", "test").replace("@ARCH@", "amd64")
        package = self.root / "suite.run"
        package.write_bytes(header.encode() + archive.read_bytes())
        package.chmod(0o755)
        # macOS has BSD mv; emulate only GNU mv -T for this archive-logic test.
        tools = self.root / "tools"
        tools.mkdir()
        mv = tools / "mv"
        mv.write_text(f"#!{sys.executable}\nimport os,sys\na=[x for x in sys.argv[1:] if x!='-T']\nos.rename(a[0],a[1])\n")
        mv.chmod(0o755)
        env = dict(os.environ, XDG_DATA_HOME=str(self.root / "userdata"), PATH=str(tools) + os.pathsep + os.environ["PATH"])
        def extract():
            return Path(subprocess.check_output(["sh", str(package), "--extract-only"], env=env, text=True).strip())
        installed = extract()
        (installed / "program").unlink()
        (installed / "preserve-user-file").write_text("keep")
        self.assertEqual(extract(), installed)
        self.assertEqual((installed / "program").read_bytes(), b"expected program")
        backups = list(installed.parent.glob(installed.name + ".damaged.*"))
        self.assertEqual(len(backups), 1)
        self.assertEqual((backups[0] / "preserve-user-file").read_text(), "keep")

    def test_runtime_without_closure_record_is_rejected(self):
        spec = importlib.util.spec_from_file_location("linux_runtime", suite.ROOT / "packaging/linux/runtime.py")
        runtime = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(runtime)
        with self.assertRaisesRegex(RuntimeError, "incomplete"):
            runtime.audit(self.root)


if __name__ == "__main__":
    unittest.main()
