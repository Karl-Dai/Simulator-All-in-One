#!/usr/bin/env python3
"""Keep suite patch assets identical to ui/. Never modifies upstream repos.

Run --write only against the suite's disposable patched clones after reviewing
their diff. It refreshes the four patches, including reviewed adapter changes.
--check is read-only and also runs before a native suite build.
"""
import argparse
import json
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def assets(workspace, root=ROOT):
    lock = json.loads((root / "components.lock.json").read_text())
    repos = {r["id"]: workspace / r["directory"] for r in lock["repositories"]}
    for c in lock["components"]:
        for name in ("tokens.css", "components.css", "theme.ts", "Icon.vue"):
            yield root / "ui" / name, repos[c["repository"]] / c["frontend"] / "src/simlab-ui" / name
    for identity, repo in repos.items():
        if identity == "modbus":
            yield root / "ui/preferences.go", repo / "go/internal/suitehost/appearance.go"
        else:
            yield root / "ui/preferences.rs", repo / "crates/shared/simlab_preferences.rs"


def check(workspace, root=ROOT):
    for source, target in assets(workspace, root):
        if not target.is_file() or target.read_bytes() != source.read_bytes():
            raise ValueError(f"Stale shared UI asset: {target}. Refresh suite patches with scripts/sync_ui.py --write.")


def write(workspace, root=ROOT):
    lock = json.loads((root / "components.lock.json").read_text())
    for repo in lock["repositories"]:
        head = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=workspace / repo["directory"], text=True).strip()
        if head != repo["commit"]:
            raise ValueError(f"Refusing to update unlocked clone {repo['directory']}")
    generated = list(assets(workspace, root))
    for source, target in generated:
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)
    for repo in lock["repositories"]:
        path = workspace / repo["directory"]
        names = [str(target.relative_to(path)) for _, target in generated if target.is_relative_to(path)]
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
