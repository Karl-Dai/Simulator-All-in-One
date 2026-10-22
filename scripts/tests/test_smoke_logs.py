import importlib.util
from pathlib import Path
import tempfile
import unittest


SPEC = importlib.util.spec_from_file_location("smoke", Path(__file__).resolve().parents[1] / "smoke.py")
smoke = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(smoke)


class SmokeLogTests(unittest.TestCase):
    def test_tail_read_does_not_change_existing_writer_append_position(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "process.log"
            with path.open("wb", buffering=0) as writer:
                writer.write(b"first line\n")
                position = writer.tell()
                self.assertEqual(smoke.read_log_tail(path), "first line\n")
                self.assertEqual(writer.tell(), position)
                writer.write(b"second line\n")
                self.assertEqual(path.read_bytes(), b"first line\nsecond line\n")

    def test_oversized_log_returns_only_the_latest_fixed_size_tail(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "process.log"
            with path.open("wb", buffering=0) as writer:
                writer.write(b"old data" * 12000)
                first_tail = b"A" * smoke.LOG_TAIL_BYTES
                writer.write(first_tail)
                self.assertEqual(smoke.read_log_tail(path), first_tail.decode())
                writer.write(b"newest")
                tail = smoke.read_log_tail(path)
                self.assertEqual(len(tail), smoke.LOG_TAIL_BYTES)
                self.assertEqual(tail, (first_tail + b"newest")[-smoke.LOG_TAIL_BYTES:].decode())

    def test_invalid_utf8_is_replaced_instead_of_failing_diagnostics(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "process.log"
            path.write_bytes(b"message\xff\n")
            self.assertEqual(smoke.read_log_tail(path), "message\ufffd\n")


if __name__ == "__main__":
    unittest.main()
