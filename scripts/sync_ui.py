#!/usr/bin/env python3
"""Keep suite patch assets identical to ui/. Never modifies upstream repos.

Run --write only against the suite's disposable patched clones after reviewing
their diff. It refreshes the four patches, including reviewed adapter changes.
--check is read-only and also runs before a native suite build.
"""
import argparse
import json
import hashlib
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def frontend_assets(root=ROOT):
    """The single public asset list shared by synchronization and verification."""
    return sorted(path for path in (root / "ui").rglob("*")
                  if path.is_file() and path.suffix in (".ts", ".vue", ".css", ".json"))


def frontend_directories(workspace, root=ROOT):
    lock = json.loads((root / "components.lock.json").read_text())
    repos = {r["id"]: workspace / r["directory"] for r in lock["repositories"]}
    return [repos[c["repository"]] / c["frontend"] / "src/simlab-ui" for c in lock["components"]]


def public_manifest(root=ROOT):
    files = {path.relative_to(root / "ui").as_posix(): hashlib.sha256(path.read_bytes()).hexdigest()
             for path in frontend_assets(root)}
    digest = hashlib.sha256(json.dumps(files, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
    return {"schemaVersion": 1, "sourceSha256": digest, "files": files}


def assets(workspace, root=ROOT):
    lock = json.loads((root / "components.lock.json").read_text())
    repos = {r["id"]: workspace / r["directory"] for r in lock["repositories"]}
    for c in lock["components"]:
        for source in frontend_assets(root):
            yield source, repos[c["repository"]] / c["frontend"] / "src/simlab-ui" / source.relative_to(root / "ui")
        # Role assets belong to the suite, just like shared UI assets. Keep
        # browser previews and native bundles on the same checked-in identity.
        if "project" in c:
            icons = root / "assets/roles" / c["id"]
            repo = repos[c["repository"]]
            yield icons / "icon.svg", repo / c["frontend"] / "public/suite-icon.svg"
            destination = repo / c["project"] / "suite-icons"
            for name in ("32x32.png", "128x128.png", "128x128@2x.png", "icon.png", "icon.icns", "icon.ico"):
                yield icons / name, destination / name
            if c["repository"] == "modbus":
                yield icons / "128x128.png", repo / "go/internal/suitehost/icons" / (c["id"] + ".png")
    for identity, repo in repos.items():
        if identity == "modbus":
            yield root / "ui/preferences.go", repo / "go/internal/suitehost/appearance.go"
        else:
            yield root / "ui/preferences.rs", repo / "crates/shared/simlab_preferences.rs"
            yield root / "ui/contracts/appearance-cases.json", repo / "crates/shared/contracts/appearance-cases.json"


def check(workspace, root=ROOT):
    for source, target in assets(workspace, root):
        if not target.is_file() or target.read_bytes() != source.read_bytes():
            raise ValueError(f"Stale shared UI asset: {target}. Refresh suite patches with scripts/sync_ui.py --write.")
    expected = public_manifest(root)
    for directory in frontend_directories(workspace, root):
        path = directory / ".source.json"
        if not path.is_file() or json.loads(path.read_text()) != expected:
            raise ValueError(f"Stale shared UI provenance: {path}")


def write(workspace, root=ROOT):
    lock = json.loads((root / "components.lock.json").read_text())
    for repo in lock["repositories"]:
        head = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=workspace / repo["directory"], text=True).strip()
        if head != repo["commit"]:
            raise ValueError(f"Refusing to update unlocked clone {repo['directory']}")
    generated = list(assets(workspace, root))
    manifest = public_manifest(root)
    for directory in frontend_directories(workspace, root):
        record = directory / ".source.json"
        if record.exists():
            previous = json.loads(record.read_text())
            for name, digest in previous.get("files", {}).items():
                path = directory / name
                if (Path(name).is_absolute() or ".." in Path(name).parts
                        or not path.resolve().is_relative_to(directory.resolve())):
                    raise ValueError(f"Unsafe generated asset path: {name}")
                if name not in manifest["files"] and path.exists():
                    if path.is_symlink() or hashlib.sha256(path.read_bytes()).hexdigest() != digest:
                        raise ValueError(f"Refusing to remove modified generated asset: {path}")
                    path.unlink()
        directory.mkdir(parents=True, exist_ok=True)
        record.write_text(json.dumps(manifest, indent=2) + "\n")
    for source, target in generated:
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)
    for repo in lock["repositories"]:
        path = workspace / repo["directory"]
        names = [str(target.relative_to(path)) for _, target in generated if target.is_relative_to(path)]
        names += [str((directory / ".source.json").relative_to(path))
                  for directory in frontend_directories(workspace, root) if directory.is_relative_to(path)]
        subprocess.run(["git", "add", "--intent-to-add", "--", *names], cwd=path, check=True)
        diff = subprocess.check_output(["git", "diff", "HEAD", "--binary"], cwd=path)
        (root / repo["patch"]).write_bytes(diff)
    check(workspace, root)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--workspace", type=Path, default=ROOT / ".components")
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--write", action="store_true")
    mode.add_argument("--check", action="store_true")
    args = parser.parse_args()
    (write if args.write else check)(args.workspace.resolve())
    print("Shared UI assets match all eight roles.")
