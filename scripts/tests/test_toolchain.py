import contextlib
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch


SPEC = importlib.util.spec_from_file_location(
    "toolchain", Path(__file__).resolve().parents[1] / "toolchain.py"
)
toolchain = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(toolchain)

SCRIPT = Path(__file__).resolve().parents[1] / "toolchain.py"


class ToolchainTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.bin = self.root / "bin"
        self.bin.mkdir()

    def make_tool(self, name):
        path = self.bin / name
        path.write_text("")
        return str(path.resolve())

    def make_lock(self, tools):
        return {"schemaVersion": 1, "tools": tools}

    def write_lock(self, tools, name="toolchain.lock.json"):
        path = self.root / name
        path.write_text(json.dumps({"schemaVersion": 1, "tools": tools}), encoding="utf-8")
        return path

    def which_patch(self, mapping):
        return patch.object(
            toolchain.shutil, "which", side_effect=lambda command: mapping.get(command)
        )

    def runner(self, outputs, statuses=None, errors=None, calls=None):
        statuses = statuses or {}
        errors = errors or {}

        def fake_run(argv, **kwargs):
            if calls is not None:
                calls.append(list(argv))
            self.assertFalse(kwargs.get("shell"))
            self.assertNotIn("env", kwargs)
            self.assertGreater(kwargs.get("timeout"), 0)
            self.assertLessEqual(kwargs.get("timeout"), 60)
            key = Path(argv[0]).name
            if str(Path(argv[0]).resolve()) == str(Path(sys.executable).resolve()):
                key = "python"
            return subprocess.CompletedProcess(
                argv,
                statuses.get(key, 0),
                outputs.get(key, b""),
                errors.get(key, b""),
            )

        return fake_run

    def test_correct_versions_and_selection(self):
        lock = self.make_lock({
            "node": {"version": "22.0.0"},
            "npm": {"version": "10.0.0"},
            "python": {"version": "3.12.0"},
            "go": {"version": "1.27.0"},
            "rust": {"version": "1.99.0"},
            "wails": {"version": "2.14.0"},
            "nsis": {"version": "3.11", "platforms": ["windows"]},
        })
        mapping = {
            "node": self.make_tool("node"),
            "npm": self.make_tool("npm"),
            "go": self.make_tool("go"),
            "rustc": self.make_tool("rustc"),
            "wails": self.make_tool("wails"),
        }
        outputs = {
            "node": b"v22.0.0\n",
            "npm": b"10.0.0\n",
            "python": b"Python 3.12.0\n",
            "go": b"go version go1.27.0 darwin/arm64\n",
            "rustc": b"rustc 1.99.0 (abcdef 2025-01-01)\n",
            "wails": b"v2.14.0\n",
        }
        with self.which_patch(mapping), patch.object(
            toolchain.subprocess, "run", side_effect=self.runner(outputs)
        ):
            records = toolchain.inspect_tools(lock, platform="darwin")
            checked = toolchain.check_tools(lock, platform="darwin")

        self.assertEqual(
            [record["tool"] for record in records],
            ["node", "npm", "python", "go", "rust", "wails"],
        )
        self.assertTrue(all(record["matches"] for record in records), records)
        self.assertEqual([record["actual"] for record in records],
                         ["22.0.0", "10.0.0", "3.12.0", "1.27.0", "1.99.0", "2.14.0"])
        for record in records:
            self.assertTrue(Path(record["path"]).is_absolute())
        self.assertEqual(records, checked)
        json.dumps(records)  # records must be JSON serializable

    def test_python_version_may_arrive_on_stderr(self):
        lock = self.make_lock({"python": {"version": "3.12.0"}})
        outputs = {"python": b""}
        errors = {"python": b"Python 3.12.0\n"}
        with patch.object(
            toolchain.subprocess, "run", side_effect=self.runner(outputs, errors=errors)
        ):
            records = toolchain.inspect_tools(lock, platform="linux")
        self.assertTrue(records[0]["matches"])
        self.assertEqual(records[0]["actual"], "3.12.0")

    def test_noisy_wails_output_is_parsed(self):
        lock = self.make_lock({"wails": {"version": "2.14.0"}})
        mapping = {"wails": self.make_tool("wails")}
        noise = b"Wails CLI\n\nVersion | v2.14.0\nGo Version | go1.22.1\n"
        with self.which_patch(mapping), patch.object(
            toolchain.subprocess, "run", side_effect=self.runner({"wails": noise})
        ):
            records = toolchain.inspect_tools(lock, platform="darwin")
        self.assertTrue(records[0]["matches"])
        self.assertEqual(records[0]["actual"], "2.14.0")

    def test_windows_npm_mapping_uses_node_and_npm_cli_without_shell(self):
        lock = self.make_lock({"npm": {"version": "10.0.0"}})
        npm = Path(self.make_tool("npm.cmd"))
        node = Path(self.make_tool("node.exe"))
        cli = npm.parent / "node_modules" / "npm" / "bin" / "npm-cli.js"
        cli.parent.mkdir(parents=True)
        cli.write_text("// fixture\n")
        calls = []

        def fake_run(argv, **kwargs):
            calls.append(argv)
            self.assertFalse(kwargs["shell"])
            return subprocess.CompletedProcess(argv, 0, b"10.0.0\n", b"")

        with self.which_patch({"npm": str(npm), "node": str(node)}), patch.object(
            toolchain.subprocess, "run", side_effect=fake_run
        ):
            records = toolchain.inspect_tools(lock, platform="windows")

        self.assertTrue(records[0]["matches"])
        self.assertEqual(records[0]["path"], str(npm.resolve()))
        self.assertEqual(calls, [[str(node.resolve()), str(cli), "--version"]])

    def test_windows_nsis_falls_back_to_slash_version(self):
        lock = self.make_lock({"nsis": {"version": "3.11", "platforms": ["windows"]}})
        mapping = {"makensis": self.make_tool("makensis.exe")}
        calls = []

        def fake_run(argv, **kwargs):
            calls.append(list(argv))
            if argv[-1] == "/VERSION":
                return subprocess.CompletedProcess(argv, 0, b"v3.11\n", b"")
            return subprocess.CompletedProcess(argv, 1, b"", b"invalid option")

        with self.which_patch(mapping), patch.object(
            toolchain.subprocess, "run", side_effect=fake_run
        ):
            records = toolchain.inspect_tools(lock, names=["nsis"], platform="windows")

        self.assertTrue(records[0]["matches"])
        self.assertEqual([call[-1] for call in calls], ["-VERSION", "/VERSION"])

    def test_missing_executable_is_diagnosed(self):
        lock = self.make_lock({"node": {"version": "22.0.0"}})
        with self.which_patch({}), patch.object(
            toolchain.subprocess, "run", side_effect=AssertionError("must not run")
        ):
            records = toolchain.inspect_tools(lock, platform="darwin")
            with self.assertRaisesRegex(toolchain.ToolchainError, "not installed"):
                toolchain.check_tools(lock, platform="darwin")
        self.assertFalse(records[0]["matches"])
        self.assertIsNone(records[0]["path"])
        self.assertIn("not installed", records[0]["error"])

    def test_nonzero_status_and_output_are_reported(self):
        lock = self.make_lock({"node": {"version": "22.0.0"}})
        mapping = {"node": self.make_tool("node")}
        with self.which_patch(mapping), patch.object(
            toolchain.subprocess,
            "run",
            side_effect=self.runner({}, statuses={"node": 9}, errors={"node": b"boom"}),
        ):
            records = toolchain.inspect_tools(lock, platform="darwin")
            with self.assertRaisesRegex(toolchain.ToolchainError, "status 9.*boom"):
                toolchain.check_tools(lock, platform="darwin")
        self.assertIn("status 9", records[0]["error"])

    def test_timeout_is_diagnosed(self):
        lock = self.make_lock({"go": {"version": "1.27.0"}})
        mapping = {"go": self.make_tool("go")}
        with self.which_patch(mapping), patch.object(
            toolchain.subprocess,
            "run",
            side_effect=subprocess.TimeoutExpired(["go"], toolchain.COMMAND_TIMEOUT),
        ):
            records = toolchain.inspect_tools(lock, platform="linux")
            with self.assertRaisesRegex(toolchain.ToolchainError, "timed out"):
                toolchain.check_tools(lock, platform="linux")
        self.assertIn("timed out", records[0]["error"])

    def test_mismatch_reports_expected_and_actual(self):
        lock = self.make_lock({"node": {"version": "22.0.0"}})
        mapping = {"node": self.make_tool("node")}
        with self.which_patch(mapping), patch.object(
            toolchain.subprocess,
            "run",
            side_effect=self.runner({"node": b"v18.2.0\n"}),
        ):
            records = toolchain.inspect_tools(lock, platform="darwin")
            with self.assertRaisesRegex(
                toolchain.ToolchainError, "node: expected 22.0.0, found 18.2.0"
            ):
                toolchain.check_tools(lock, platform="darwin")
        self.assertFalse(records[0]["matches"])
        self.assertEqual(records[0]["actual"], "18.2.0")

    def test_unparseable_output_is_diagnosed(self):
        lock = self.make_lock({"node": {"version": "22.0.0"}})
        mapping = {"node": self.make_tool("node")}
        with self.which_patch(mapping), patch.object(
            toolchain.subprocess, "run", side_effect=self.runner({"node": b"command failed\n"})
        ):
            records = toolchain.inspect_tools(lock, platform="darwin")
        self.assertIn("Cannot parse node version", records[0]["error"])

    def test_nsis_is_skipped_by_default_and_rejected_when_selected(self):
        lock = self.make_lock({
            "node": {"version": "22.0.0"},
            "nsis": {"version": "3.11", "platforms": ["windows"]},
        })
        mapping = {"node": self.make_tool("node")}
        with self.which_patch(mapping), patch.object(
            toolchain.subprocess, "run", side_effect=self.runner({"node": b"22.0.0\n"})
        ):
            records = toolchain.inspect_tools(lock, platform="darwin")
            with self.assertRaisesRegex(
                toolchain.ToolchainError, "only supported on windows"
            ):
                toolchain.inspect_tools(lock, names=["nsis"], platform="darwin")
        self.assertEqual([record["tool"] for record in records], ["node"])

    def test_selection_preserves_requested_order_and_accepts_comma_string(self):
        lock = self.make_lock({
            "node": {"version": "22.0.0"},
            "go": {"version": "1.27.0"},
        })
        mapping = {"node": self.make_tool("node"), "go": self.make_tool("go")}
        outputs = {"node": b"22.0.0\n", "go": b"go version go1.27.0 linux/amd64\n"}
        with self.which_patch(mapping), patch.object(
            toolchain.subprocess, "run", side_effect=self.runner(outputs)
        ):
            records = toolchain.inspect_tools(lock, names="go,node", platform="linux")
        self.assertEqual([record["tool"] for record in records], ["go", "node"])

    def test_unknown_or_missing_selection_is_rejected(self):
        lock = self.make_lock({"node": {"version": "22.0.0"}})
        with self.assertRaisesRegex(toolchain.ToolchainError, "Unknown tool 'ruby'"):
            toolchain.inspect_tools(lock, names=["ruby"])
        with self.assertRaisesRegex(toolchain.ToolchainError, "not defined"):
            toolchain.inspect_tools(lock, names=["go"])
        with self.assertRaisesRegex(toolchain.ToolchainError, "No tools selected"):
            toolchain.inspect_tools(lock, names=[])

    def test_unsupported_platform_is_rejected(self):
        lock = self.make_lock({"node": {"version": "22.0.0"}})
        with self.assertRaisesRegex(toolchain.ToolchainError, "Unsupported platform"):
            toolchain.inspect_tools(lock, platform="freebsd")
        self.assertEqual(toolchain.normalize_platform("linux2"), "linux")
        self.assertEqual(toolchain.normalize_platform("win32"), "windows")
        self.assertEqual(toolchain.normalize_platform("darwin"), "darwin")

    def test_schema_and_lock_validation(self):
        cases = [
            ({"tools": {"node": {"version": "22.0.0"}}}, "missing schemaVersion"),
            ({"schemaVersion": 2, "tools": {"node": {"version": "22.0.0"}}},
             "Unsupported toolchain schema version"),
            ({"schemaVersion": "1", "tools": {"node": {"version": "22.0.0"}}},
             "Invalid toolchain schemaVersion"),
            ({"schemaVersion": 1, "tools": []}, "tools object"),
            ({"schemaVersion": 1, "tools": {}}, "contains no tools"),
            ({"schemaVersion": 1, "tools": {"ruby": {"version": "3.3.0"}}},
             "Unknown tool"),
            ({"schemaVersion": 1, "tools": {"node": {}}}, "missing a version"),
            ({"schemaVersion": 1, "tools": {"node": {"version": 22}}}, "version"),
            ({"schemaVersion": 1, "tools": {"node": {"version": "22.0\n0"}}},
             "control characters"),
            ({"schemaVersion": 1, "tools": {"nsis": {"version": "3.11",
                                                    "platforms": ["solaris"]}}},
             "unsupported platform"),
            ({"schemaVersion": 1, "tools": {"nsis": {"version": "3.11",
                                                    "platforms": "windows"}}},
             "non-empty list"),
        ]
        for index, (document, pattern) in enumerate(cases):
            path = self.root / f"bad{index}.json"
            path.write_text(json.dumps(document), encoding="utf-8")
            with self.assertRaisesRegex(toolchain.ToolchainError, pattern):
                toolchain.load_lock(path)

    def test_malformed_json_and_missing_file(self):
        path = self.root / "broken.json"
        path.write_text("{not json", encoding="utf-8")
        with self.assertRaisesRegex(toolchain.ToolchainError, "Invalid JSON"):
            toolchain.load_lock(path)
        with self.assertRaisesRegex(toolchain.ToolchainError, "Cannot read"):
            toolchain.load_lock(self.root / "absent.json")

    def test_digest_is_canonical_and_tracks_versions(self):
        first = self.write_lock({
            "node": {"version": "22.0.0"},
            "nsis": {"version": "3.11", "platforms": ["windows", "linux"]},
        })
        digest = toolchain.lock_digest(first)
        self.assertEqual(
            digest,
            hashlib.sha256(
                json.dumps(
                    toolchain.load_lock(first),
                    sort_keys=True,
                    separators=(",", ":"),
                    ensure_ascii=False,
                ).encode("utf-8")
            ).hexdigest(),
        )

        reformatted = self.root / "reformatted.json"
        reformatted.write_text(
            json.dumps(
                {
                    "tools": {
                        "nsis": {"platforms": ["linux", "windows"], "version": "3.11"},
                        "node": {"version": "22.0.0"},
                    },
                    "schemaVersion": 1,
                },
                indent=4,
            ),
            encoding="utf-8",
        )
        self.assertEqual(toolchain.lock_digest(reformatted), digest)

        changed = self.write_lock({
            "node": {"version": "22.1.0"},
            "nsis": {"version": "3.11", "platforms": ["windows", "linux"]},
        }, name="changed.json")
        self.assertNotEqual(toolchain.lock_digest(changed), digest)

    def test_ci_writes_locked_versions_without_running_tools(self):
        lock = self.write_lock({
            "node": {"version": "22.0.0"},
            "npm": {"version": "10.0.0"},
            "nsis": {"version": "3.11", "platforms": ["windows"]},
        })
        output = self.root / "github.output"
        with patch.object(
            toolchain.subprocess, "run", side_effect=AssertionError("ci must not run tools")
        ):
            code = toolchain.main(
                ["--lock", str(lock), "ci", "--output", str(output), "--tools", "node,npm"]
            )
        self.assertEqual(code, 0)
        self.assertEqual(output.read_text(encoding="utf-8"), "node=22.0.0\nnpm=10.0.0\n")

    def test_ci_rejects_control_characters_in_output_path(self):
        lock = self.write_lock({"node": {"version": "22.0.0"}})
        stderr = io.StringIO()
        with contextlib.redirect_stderr(stderr):
            code = toolchain.main(
                ["--lock", str(lock), "ci", "--output", str(self.root / "bad\nname")]
            )
        self.assertEqual(code, 1)
        self.assertIn("control characters", stderr.getvalue())

    def test_ci_rejects_control_characters_in_lock_versions(self):
        path = self.root / "injected.json"
        path.write_text(
            json.dumps({"schemaVersion": 1, "tools": {"node": {"version": "22.0.0\nX=1"}}}),
            encoding="utf-8",
        )
        stderr = io.StringIO()
        with contextlib.redirect_stderr(stderr):
            code = toolchain.main(
                ["--lock", str(path), "ci", "--output", str(self.root / "out")]
            )
        self.assertEqual(code, 1)
        self.assertIn("control characters", stderr.getvalue())

    def test_record_writes_actual_versions_digest_and_platform(self):
        lock = self.write_lock({"node": {"version": "22.0.0"}})
        mapping = {"node": self.make_tool("node")}
        output = self.root / "toolchain-record.json"
        with self.which_patch(mapping), patch.object(
            toolchain.subprocess, "run", side_effect=self.runner({"node": b"v22.0.0\n"})
        ):
            code = toolchain.main(
                ["--lock", str(lock), "--tools", "node", "record", "--output", str(output)]
            )
        self.assertEqual(code, 0)
        document = json.loads(output.read_text(encoding="utf-8"))
        self.assertEqual(document["schemaVersion"], 1)
        self.assertEqual(document["platform"], toolchain.normalize_platform())
        self.assertEqual(document["lockSha256"], toolchain.lock_digest(lock))
        self.assertEqual(document["tools"], {"node": "22.0.0"})

    def test_record_does_not_write_when_check_fails(self):
        lock = self.write_lock({"node": {"version": "22.0.0"}})
        mapping = {"node": self.make_tool("node")}
        output = self.root / "toolchain-record.json"
        stderr = io.StringIO()
        with self.which_patch(mapping), patch.object(
            toolchain.subprocess, "run", side_effect=self.runner({"node": b"18.2.0\n"})
        ), contextlib.redirect_stderr(stderr):
            code = toolchain.main(
                ["--lock", str(lock), "record", "--output", str(output), "--tools", "node"]
            )
        self.assertEqual(code, 1)
        self.assertFalse(output.exists())
        self.assertIn("expected 22.0.0", stderr.getvalue())

    def test_check_command_reports_matches_and_failures(self):
        lock = self.write_lock({"node": {"version": "22.0.0"}})
        mapping = {"node": self.make_tool("node")}
        stdout = io.StringIO()
        with self.which_patch(mapping), patch.object(
            toolchain.subprocess, "run", side_effect=self.runner({"node": b"22.0.0\n"})
        ), contextlib.redirect_stdout(stdout):
            self.assertEqual(toolchain.main(["--lock", str(lock), "check"]), 0)
        self.assertIn("node: 22.0.0", stdout.getvalue())

    def test_wails_fallback_uses_first_of_multi_root_gopath(self):
        lock = self.make_lock({"wails": {"version": "2.14.0"}})
        go = self.make_tool("go")
        first = self.root / "gopath-one"
        second = self.root / "gopath-two"
        binaries = []
        for root in (first, second):
            binary = root / "bin" / "wails"
            binary.parent.mkdir(parents=True)
            binary.write_text("")
            binaries.append(binary)
        output = (str(first) + os.pathsep + str(second) + "\n").encode()
        calls = []

        def fake_run(argv, **kwargs):
            calls.append(list(argv))
            self.assertFalse(kwargs.get("shell"))
            self.assertNotIn("env", kwargs)
            self.assertGreater(kwargs.get("timeout"), 0)
            self.assertLessEqual(kwargs.get("timeout"), 60)
            if argv[1:] == ["env", "GOPATH"]:
                return subprocess.CompletedProcess(argv, 0, output, b"")
            self.assertEqual(argv, [os.path.abspath(binaries[0]), "version"])
            return subprocess.CompletedProcess(argv, 0, b"v2.14.0\n", b"")

        with self.which_patch({"go": go}), patch.object(
            toolchain.subprocess, "run", side_effect=fake_run
        ):
            records = toolchain.inspect_tools(lock, platform="darwin")

        self.assertTrue(records[0]["matches"], records[0])
        self.assertEqual(records[0]["path"], os.path.abspath(binaries[0]))
        self.assertEqual(
            calls,
            [[os.path.abspath(go), "env", "GOPATH"],
             [os.path.abspath(binaries[0]), "version"]],
        )

    def test_wails_on_path_does_not_consult_gopath(self):
        lock = self.make_lock({"wails": {"version": "2.14.0"}})
        wails = self.make_tool("wails")
        calls = []

        def fake_run(argv, **kwargs):
            calls.append(list(argv))
            self.assertNotIn("env", kwargs)
            return subprocess.CompletedProcess(argv, 0, b"v2.14.0\n", b"")

        with self.which_patch({"wails": wails}), patch.object(
            toolchain.subprocess, "run", side_effect=fake_run
        ):
            records = toolchain.inspect_tools(lock, platform="darwin")

        self.assertTrue(records[0]["matches"])
        self.assertEqual(calls, [[str(Path(wails).resolve()), "version"]])

    def test_wails_fallback_reports_missing_binary(self):
        lock = self.make_lock({"wails": {"version": "2.14.0"}})
        go = self.make_tool("go")
        gopath = self.root / "empty-gopath"
        gopath.mkdir()

        def fake_run(argv, **kwargs):
            return subprocess.CompletedProcess(argv, 0, (str(gopath) + "\n").encode(), b"")

        with self.which_patch({"go": go}), patch.object(
            toolchain.subprocess, "run", side_effect=fake_run
        ):
            records = toolchain.inspect_tools(lock, platform="linux")
            with self.assertRaisesRegex(toolchain.ToolchainError, "Wails CLI is not installed"):
                toolchain.check_tools(lock, platform="linux")

        self.assertFalse(records[0]["matches"])
        self.assertIn("Wails CLI is not installed at", records[0]["error"])
        self.assertIn(str(gopath / "bin" / "wails"), records[0]["error"])

    def test_wails_fallback_reports_missing_go(self):
        lock = self.make_lock({"wails": {"version": "2.14.0"}})
        with self.which_patch({}), patch.object(
            toolchain.subprocess, "run", side_effect=AssertionError("must not run")
        ):
            records = toolchain.inspect_tools(lock, platform="darwin")
        self.assertFalse(records[0]["matches"])
        self.assertIn("needs go", records[0]["error"])
        self.assertIn("not installed", records[0]["error"])

    def test_wails_fallback_go_env_failure_is_bounded_and_reported(self):
        lock = self.make_lock({"wails": {"version": "2.14.0"}})
        go = self.make_tool("go")
        calls = []

        def fake_run(argv, **kwargs):
            calls.append(list(argv))
            self.assertFalse(kwargs.get("shell"))
            self.assertGreater(kwargs.get("timeout"), 0)
            raise subprocess.TimeoutExpired(argv, kwargs["timeout"])

        with self.which_patch({"go": go}), patch.object(
            toolchain.subprocess, "run", side_effect=fake_run
        ):
            records = toolchain.inspect_tools(lock, platform="darwin")

        self.assertEqual(len(calls), 1)
        self.assertFalse(records[0]["matches"])
        self.assertIn("Cannot resolve Wails through GOPATH", records[0]["error"])
        self.assertIn("timed out", records[0]["error"])

    def test_nsis_normalizes_trailing_zero_components(self):
        lock = self.make_lock({"nsis": {"version": "3.13.0", "platforms": ["windows"]}})
        mapping = {"makensis": self.make_tool("makensis.exe")}
        with self.which_patch(mapping), patch.object(
            toolchain.subprocess, "run", side_effect=self.runner({"makensis.exe": b"v3.13\n"})
        ):
            records = toolchain.inspect_tools(lock, names=["nsis"], platform="windows")
        self.assertTrue(records[0]["matches"], records[0])
        self.assertEqual(records[0]["actual"], "3.13")

    def test_nsis_prerelease_stays_strict(self):
        lock = self.make_lock({"nsis": {"version": "3.13.0-rc.1", "platforms": ["windows"]}})
        mapping = {"makensis": self.make_tool("makensis.exe")}
        with self.which_patch(mapping), patch.object(
            toolchain.subprocess, "run", side_effect=self.runner({"makensis.exe": b"v3.13\n"})
        ):
            records = toolchain.inspect_tools(lock, names=["nsis"], platform="windows")
        self.assertFalse(records[0]["matches"])
        self.assertIn("version mismatch", records[0]["error"])

    def test_non_nsis_versions_stay_strict(self):
        lock = self.make_lock({"node": {"version": "22.0.0"}})
        mapping = {"node": self.make_tool("node")}
        with self.which_patch(mapping), patch.object(
            toolchain.subprocess, "run", side_effect=self.runner({"node": b"v22.0\n"})
        ):
            records = toolchain.inspect_tools(lock, platform="darwin")
        self.assertFalse(records[0]["matches"])
        self.assertIn("version mismatch", records[0]["error"])

    def test_symlinked_executable_keeps_its_argv0_dispatch(self):
        lock = self.make_lock({"rust": {"version": "1.99.0"}})
        if os.name == "nt":
            target = self.bin / "rustup.cmd"
            target.write_text(
                "@echo off\r\n"
                "if /I \"%~n0\"==\"rustc\" (\r\n"
                "  echo rustc 1.99.0\r\n"
                ") else (\r\n"
                "  echo rustup 1.28.0\r\n"
                ")\r\n",
                encoding="ascii",
            )
            link = self.bin / "rustc.cmd"
        else:
            target = self.bin / "rustup"
            target.write_text(
                "#!/usr/bin/env python3\n"
                "import os, sys\n"
                "name = os.path.basename(sys.argv[0])\n"
                "print('rustc 1.99.0' if name == 'rustc' else 'rustup 1.28.0')\n",
                encoding="utf-8",
            )
            target.chmod(0o755)
            link = self.bin / "rustc"
        try:
            os.symlink(target, link)
        except (OSError, NotImplementedError) as error:
            self.skipTest(f"Cannot create a symlink on this system: {error}")

        environment = {"PATH": os.pathsep.join([str(self.bin), os.environ.get("PATH", "")])}
        if os.name == "nt":
            environment["PATHEXT"] = ".CMD;" + os.environ.get("PATHEXT", "")
        with patch.dict(os.environ, environment):
            records = toolchain.inspect_tools(
                lock, platform=toolchain.normalize_platform()
            )

        record = records[0]
        self.assertTrue(record["matches"], record)
        self.assertEqual(record["actual"], "1.99.0")
        self.assertEqual(os.path.normcase(record["path"]), os.path.normcase(os.path.abspath(link)))
        self.assertNotEqual(record["path"], os.path.abspath(target))
        self.assertTrue(os.path.samefile(record["path"], target))

    def test_help_works_without_a_lockfile(self):
        result = subprocess.run(
            [sys.executable, str(SCRIPT), "--lock", str(self.root / "absent.json"), "--help"],
            capture_output=True,
            text=True,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("usage", result.stdout.lower())


if __name__ == "__main__":
    unittest.main()
