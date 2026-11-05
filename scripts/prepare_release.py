"""Verify the complete native artifact set and prepare a GitHub release."""
import hashlib
import json
from pathlib import Path
import re
import shutil
import sys


def prepare(tag, artifacts, output, root=Path(__file__).resolve().parents[1]):
    version = json.loads((root / 'components.lock.json').read_text())['suiteVersion']
    if tag != 'v' + version or not re.fullmatch(r'v\d+\.\d+\.\d+', tag):
        raise ValueError('Tag must match the locked suite version')
    suffixes = ('darwin-arm64.dmg', 'darwin-arm64.zip', 'darwin-amd64.dmg',
                'darwin-amd64.zip', 'windows-amd64-setup.exe', 'linux-amd64.run')
    expected = {f'SAIO_{version}_{suffix}' for suffix in suffixes}
    verified = {}
    for name in sorted(expected):
        matches = list(artifacts.rglob(name))
        if len(matches) != 1:
            raise ValueError(f'Expected exactly one package: {name}, found {len(matches)}')
        package = matches[0]
        receipt = json.loads(package.with_name(name + '.sha256.json').read_text())
        digest = hashlib.sha256()
        with package.open('rb') as stream:
            for block in iter(lambda: stream.read(1024 * 1024), b''):
                digest.update(block)
        if not package.stat().st_size or receipt != {'file': name, 'sha256': digest.hexdigest()}:
            raise ValueError(f'Package checksum mismatch: {name}')
        verified[name] = (package, digest.hexdigest())
    changelog = (root / 'CHANGELOG.md').read_text()
    section = re.search(rf'^## \[{re.escape(version)}\] - .*?(?=^## |\Z)', changelog, re.M | re.S)
    if not section:
        raise ValueError('Missing changelog section')
    if output.exists():
        raise ValueError('Output directory already exists')
    output.mkdir(parents=True)
    for name, (package, _) in verified.items():
        shutil.copy2(package, output / name)
        shutil.copy2(package.with_name(name + '.sha256.json'), output / (name + '.sha256.json'))
    (output / 'SHA256SUMS').write_text(''.join(f'{value[1]}  {name}\n' for name, value in verified.items()))
    rows = '\n'.join(f'| {suffix} | [{name}](https://github.com/Karl-Dai/Simulator-All-in-One/releases/download/{tag}/{name}) |'
                     for suffix, name in zip(suffixes, (f'SAIO_{version}_{s}' for s in suffixes)))
    notes = f'# Simulator All-in-One {tag}\n\n| Package / 安装包 | Download / 下载 |\n| --- | --- |\n{rows}\n\nVerify downloads using `SHA256SUMS`. / 使用 `SHA256SUMS` 核对下载文件。\n\n{section.group(0).strip()}\n\n[Full changelog / 完整记录](https://github.com/Karl-Dai/Simulator-All-in-One/blob/{tag}/CHANGELOG.md)\n'
    (output / 'RELEASE_BODY.md').write_text(notes)
    print(f'Verified {len(verified)} packages for {tag}')


if __name__ == '__main__':
    prepare(sys.argv[1], Path(sys.argv[2]), Path(sys.argv[3]))
