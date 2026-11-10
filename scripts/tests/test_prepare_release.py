import hashlib
import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from prepare_release import prepare


class ReleaseTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.artifacts = self.root / 'artifacts'
        self.artifacts.mkdir()
        self.output = self.root / 'output'
        (self.root / 'components.lock.json').write_text('{"suiteVersion":"0.1.1"}')
        (self.root / 'CHANGELOG.md').write_text('## [0.1.1] - 2026-11-05\nCurrent notes\n\n## [0.1.0] - 2026-10-30\nOld notes\n')
        for suffix in ('darwin-arm64.dmg', 'darwin-arm64.zip', 'darwin-amd64.dmg', 'darwin-amd64.zip', 'windows-amd64-portable.exe', 'linux-amd64.run'):
            package = self.artifacts / ('SAIO_0.1.1_' + suffix)
            package.write_bytes(suffix.encode())
            package.with_name(package.name + '.sha256.json').write_text(json.dumps({'file': package.name, 'sha256': hashlib.sha256(package.read_bytes()).hexdigest()}))

    def run_prepare(self):
        prepare('v0.1.1', self.artifacts, self.output, self.root)

    def test_complete_set_and_scoped_notes(self):
        self.run_prepare()
        self.assertEqual(len((self.output / 'SHA256SUMS').read_text().splitlines()), 6)
        notes = (self.output / 'RELEASE_BODY.md').read_text()
        self.assertIn('Current notes', notes)
        self.assertNotIn('Old notes', notes)
        self.assertEqual(notes.count('/releases/download/v0.1.1/'), 6)

    def test_corruption_blocks_output(self):
        next(self.artifacts.glob('*.dmg')).write_bytes(b'corrupt')
        with self.assertRaisesRegex(ValueError, 'checksum mismatch'):
            self.run_prepare()
        self.assertFalse(self.output.exists())

    def test_missing_target_blocks_output(self):
        next(self.artifacts.glob('*.run')).unlink()
        with self.assertRaisesRegex(ValueError, 'exactly one package'):
            self.run_prepare()
        self.assertFalse(self.output.exists())

    def test_wrong_tag_blocks_output(self):
        with self.assertRaisesRegex(ValueError, 'Tag must match'):
            prepare('v0.1.2', self.artifacts, self.output, self.root)
        self.assertFalse(self.output.exists())
