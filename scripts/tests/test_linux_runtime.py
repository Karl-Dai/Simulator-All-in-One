import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch


SPEC = importlib.util.spec_from_file_location("linux_runtime", Path(__file__).resolve().parents[2] / "packaging/linux/runtime.py")
runtime = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(runtime)


class LinuxRuntimeTests(unittest.TestCase):
    def test_wayland_ui_helpers_must_be_bundled_and_audited(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            stage = root / "suite"
            host = root / "host"
            host.mkdir()
            (stage / "lib").mkdir(parents=True)

            def binary(path, label):
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(b"\x7fELF" + label.encode())
                return path

            launcher = binary(stage / "SimLab", "launcher")
            binary(stage / "lib/libwebkit2gtk-4.1.so.0", "relocated WebKit")
            helpers = stage / "runtime/usr/lib/x86_64-linux-gnu/webkit2gtk-4.1"
            for name in ("WebKitWebProcess", "WebKitNetworkProcess", "injected-bundle/libwebkit2gtkinjectedbundle.so"):
                binary(helpers / name, name)
            cursor = binary(host / "libwayland-cursor.so.0", "cursor helper")
            egl = binary(host / "libwayland-egl.so.1", "EGL window helper")
            client = binary(host / "libwayland-client.so.0", "host display transport")
            driver = binary(host / "libEGL.so.1", "host graphics interface")

            def resolve(path, env):
                requested = []
                if path.name == "SimLab":
                    requested = [cursor, egl, client, driver]
                elif path.name == cursor.name:
                    requested = [client]
                return [stage / "lib" / item.name if (stage / "lib" / item.name).exists() else item for item in requested]

            # Exercise the same audit and copy loop used by actual packaging;
            # only ELF dependency discovery/readelf are simulated on macOS.
            with patch.object(runtime, "dependencies", side_effect=resolve), patch.object(runtime, "command", return_value=""):
                with self.assertRaisesRegex(RuntimeError, "leaks a host dependency.*libwayland-cursor"):
                    runtime.audit(stage, require_record=False)
                copied = runtime.bundle_dependencies(stage, [launcher])
                self.assertEqual(copied, {cursor, egl})
                self.assertEqual((stage / "lib" / cursor.name).read_bytes(), cursor.read_bytes())
                self.assertEqual((stage / "lib" / egl.name).read_bytes(), egl.read_bytes())
                self.assertFalse((stage / "lib" / client.name).exists())
                self.assertFalse((stage / "lib" / driver.name).exists())
                self.assertEqual(runtime.audit(stage, require_record=False), [driver.name, client.name])
                (stage / "lib" / egl.name).unlink()
                with self.assertRaisesRegex(RuntimeError, "leaks a host dependency.*libwayland-egl"):
                    runtime.audit(stage, require_record=False)


if __name__ == "__main__":
    unittest.main()
