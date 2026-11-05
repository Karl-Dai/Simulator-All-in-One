"""Native build diagnostics preserve failures and never inspect arbitrary env vars."""
import copy
import json
from pathlib import Path
import subprocess
import sys
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import suite


class NativeEnvironmentTests(unittest.TestCase):
    def test_macos_versions_use_bounded_shell_free_commands(self):
        outputs = {
            ("cargo", "--version"): "cargo 1.99.1 (fixture)\n",
            ("xcrun", "--show-sdk-version"): "15.5\n",
            ("xcodebuild", "-version"): "Xcode 16.4\nBuild version 16F6\n",
            ("clang", "--version"): "Apple clang version 17.0.0\nTarget: arm64-apple-darwin\n",
        }

        def command(argv, **kwargs):
            self.assertFalse(kwargs["shell"])
            self.assertEqual(kwargs["timeout"], 10)
            self.assertEqual(kwargs["stdin"], subprocess.DEVNULL)
            self.assertNotIn("env", kwargs)
            return subprocess.CompletedProcess(argv, 0, outputs[tuple(argv)], "")

        with patch.object(suite.shutil, "which", side_effect=lambda name: name), \
                patch.object(suite.subprocess, "run", side_effect=command) as run:
            record = suite.inspect_native_environment("darwin")
        self.assertEqual(record["sdk"]["value"], "15.5")
        self.assertEqual(record["xcode"]["value"], "Xcode 16.4\nBuild version 16F6")
        self.assertEqual(record["clang"]["value"], "Apple clang version 17.0.0")
        self.assertEqual(run.call_count, 4)

    def test_missing_timeout_and_nonzero_results_are_retained(self):
        def command(argv, **kwargs):
            if argv[0] == "cargo":
                raise subprocess.TimeoutExpired(argv, kwargs["timeout"])
            return subprocess.CompletedProcess(argv, 1, "", "Package not found\n")

        with patch.object(suite.shutil, "which", side_effect=lambda name: name), \
                patch.object(suite.subprocess, "run", side_effect=command):
            record = suite.inspect_native_environment("linux")
        self.assertIn("timed out", record["cargo"]["error"])
        self.assertEqual(record["gtk"]["returncode"], 1)
        self.assertEqual(record["gtk"]["value"], "Package not found")
        self.assertIn("status 1", record["webkitgtk"]["error"])
        with patch.object(suite.shutil, "which", return_value=None), \
                patch.object(suite.subprocess, "run") as run:
            record = suite.inspect_native_environment("darwin")
        self.assertIn("not installed", record["xcode"]["error"])
        run.assert_not_called()

    def test_windows_records_only_selected_sdk_versions_and_compiler_banner(self):
        def command(argv, **kwargs):
            if argv[0] == "cl":
                return subprocess.CompletedProcess(argv, 0, "help text\n" * 500,
                                                   "Microsoft C/C++ Optimizing Compiler Version 19.51.36231 for x64\nCopyright\n")
            return subprocess.CompletedProcess(argv, 0, "cargo 1.99.1\n", "")

        with patch.dict(suite.os.environ, {"WindowsSDKVersion": "10.0.26100.0\\",
                                         "UCRTVersion": "10.0.26100.0",
                                         "UNRELATED_SECRET": "must-not-be-recorded"}, clear=True), \
                patch.object(suite.shutil, "which", side_effect=lambda name: name), \
                patch.object(suite.subprocess, "run", side_effect=command):
            record = suite.inspect_native_environment("windows")
        self.assertEqual(record["sdk"]["value"], "10.0.26100.0")
        self.assertEqual(record["ucrt"]["value"], "10.0.26100.0")
        self.assertEqual(record["msvc"]["value"],
                         "Microsoft C/C++ Optimizing Compiler Version 19.51.36231 for x64")
        self.assertNotIn("must-not-be-recorded", json.dumps(record))
        self.assertNotIn("help text", record["msvc"]["value"])

    def test_native_diagnostics_are_bound_into_build_evidence(self):
        native = {"os": "fixture", "machine": "arm64", "sdk": {"error": "not installed"}}
        with patch.object(suite.toolchain, "check_tools", return_value=[]), \
                patch.object(suite, "inspect_native_environment", return_value=native):
            record = suite.inspect_build_toolchain(suite.ROOT, "darwin", "arm64")
        self.assertEqual(record["nativeEnvironment"], native)
        original_digest = suite.build_toolchain_digest(record)
        changed = copy.deepcopy(record)
        changed["nativeEnvironment"]["sdk"] = {"value": "15.5"}
        self.assertNotEqual(suite.build_toolchain_digest(changed), original_digest)


if __name__ == "__main__":
    unittest.main()
