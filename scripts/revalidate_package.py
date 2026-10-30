#!/usr/bin/env python3
"""Reuse trusted native payloads when only packaging or validation changed."""
import argparse
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys

import suite


def allowed_change(path):
    return path in {
        "README.md", "scripts/smoke.py", "scripts/revalidate_package.py",
        "scripts/test-windows-installer.ps1", "scripts/test-windows-checker.ps1",
    } or path.startswith(("docs/", ".github/workflows/", "packaging/linux/",
                          "packaging/windows/", "scripts/tests/"))


def validate_origin(metadata, repository, changes, platform):
    if (metadata.get("repository", {}).get("full_name") != repository
            or metadata.get("head_repository", {}).get("full_name") != repository
            or metadata.get("head_branch") != "main"
            or metadata.get("event") != "workflow_dispatch"
            or metadata.get("path") != ".github/workflows/build-suite.yml"
            or metadata.get("status") != "completed"
            or not re.fullmatch(r"[0-9a-f]{40}", metadata.get("head_sha", ""))):
        raise suite.SuiteError("Only completed main-branch native suite runs from this repository may be reused")
    forbidden = [path for path in changes if not allowed_change(path)]
    if forbidden:
        raise suite.SuiteError("Compiled inputs changed; run the full native build: " + ", ".join(forbidden))
    if platform == "linux" and any(path in {
        "packaging/linux/runtime.py", "packaging/linux/run-simlab.sh",
        "packaging/linux/self-extract.sh",
    } for path in changes):
        raise suite.SuiteError("Linux package contents changed; rebuild the complete package")


def copy_declared_windows_payload(source, destination, manifest):
    files = {"suite-manifest.json"}
    for relative, entry in manifest["launcherFiles"].items():
        if entry["type"] != "file":
            raise suite.SuiteError("Windows payloads must contain regular files")
        files.add(relative)
    for component in manifest["components"]:
        for relative, entry in component["payloadFiles"].items():
            if entry["type"] != "file":
                raise suite.SuiteError("Windows payloads must contain regular files")
            files.add(component["payloadRoot"] + "/" + relative)
    destination.mkdir(parents=True)
    for relative in sorted(files):
        original = suite.safe_path(source, relative)
        target = suite.safe_path(destination, relative)
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(original, target)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-run", required=True)
    parser.add_argument("--platform", choices=("windows", "linux"), required=True)
    args = parser.parse_args()
    if not args.source_run.isdecimal():
        raise suite.SuiteError("Source run must be a numeric GitHub run ID")
    suite.native_target(args.platform, "amd64")
    repository = os.environ["GITHUB_REPOSITORY"]
    metadata = json.loads(suite.run([
        "gh", "api", f"repos/{repository}/actions/runs/{args.source_run}"
    ], capture=True))
    source_sha = metadata.get("head_sha", "")
    validate_origin(metadata, repository, [], args.platform)
    suite.run(["git", "merge-base", "--is-ancestor", source_sha, "HEAD"])
    changed = subprocess.check_output([
        "git", "diff", "--name-only", "-z", source_sha, "HEAD"
    ]).decode("utf-8").split("\0")
    validate_origin(metadata, repository, [path for path in changed if path], args.platform)
    workspace = suite.ROOT / ".validation/package-reuse"
    workspace.mkdir(parents=True, exist_ok=False)
    download = workspace / "download"
    suite.run(["gh", "run", "download", args.source_run, "--repo", repository,
               "--name", f"SAIO-{args.platform}-amd64", "--dir", download])
    suffix = "*-setup.exe" if args.platform == "windows" else "*.run"
    packages = list(download.rglob(suffix))
    if len(packages) != 1:
        raise suite.SuiteError("Expected exactly one complete native installer")
    package = packages[0]
    receipt = Path(str(package) + ".sha256.json")
    if suite.digest(package) != json.loads(receipt.read_text(encoding="utf-8"))["sha256"]:
        raise suite.SuiteError("Original installer checksum mismatch")
    stage = suite.ROOT / "dist/staging" / f"{args.platform}-amd64"
    lock = suite.read_lock()
    runtime = None
    if args.platform == "windows":
        extractor = shutil.which("7z")
        if not extractor:
            candidate = Path(os.environ.get("ProgramFiles", "C:/Program Files")) / "7-Zip/7z.exe"
            if candidate.is_file():
                extractor = str(candidate)
        if not extractor:
            raise suite.SuiteError("7-Zip is required to recover the original NSIS payloads")
        extracted = workspace / "nsis-extracted"
        suite.run([extractor, "x", "-y", f"-o{extracted}", package])
        manifests = list(extracted.rglob("suite-manifest.json"))
        runtimes = list(extracted.rglob("WebView2Runtime.exe"))
        if len(manifests) != 1 or len(runtimes) != 1:
            raise suite.SuiteError("The installer must contain one manifest and one offline WebView2 runtime")
        original = manifests[0].parent
        manifest = suite.verify_manifest(original, lock)
        if (manifest["platform"], manifest["architecture"]) != ("windows", "amd64"):
            raise suite.SuiteError("Wrong prebuilt package target")
        copy_declared_windows_payload(original, stage, manifest)
        runtime_lock = json.loads((suite.ROOT / "packaging/windows/webview2.lock.json").read_text(encoding="utf-8"))
        runtime = runtimes[0]
    else:
        data = workspace / "extracted-data"
        env = dict(os.environ, XDG_DATA_HOME=str(data))
        original = Path(suite.run([
            "timeout", "--kill-after=10s", "3m", "sh", package, "--extract-only"
        ], env=env, capture=True))
        if not original.resolve().is_relative_to(data.resolve()):
            raise suite.SuiteError("Installer extracted outside the isolated data directory")
        manifest = suite.verify_manifest(original, lock)
        if (manifest["platform"], manifest["architecture"]) != ("linux", "amd64"):
            raise suite.SuiteError("Wrong prebuilt package target")
        stage.parent.mkdir(parents=True, exist_ok=True)
        shutil.copytree(original, stage, symlinks=True)
        (stage / ".complete").unlink(missing_ok=True)
        output = suite.ROOT / "dist/packages"
        output.mkdir(parents=True, exist_ok=True)
        shutil.copy2(package, output / package.name)
        shutil.copy2(receipt, output / receipt.name)
    manifest = suite.verify_manifest(stage, lock)
    entries = [stage / ("SAIO.exe" if args.platform == "windows" else "SAIO")]
    entries += [stage / item["executable"] for item in manifest["components"]]
    for entry in entries:
        suite.assert_architecture(entry, args.platform, "amd64")
    if runtime is not None:
        suite.package("windows", "amd64", webview2=runtime, webview2_sha256=runtime_lock["sha256"])
    suite.write_json(suite.ROOT / "dist/provenance.json", {
        "sourceRun": args.source_run, "sourceHead": source_sha,
        "validationHead": suite.run(["git", "rev-parse", "HEAD"], capture=True),
        "platform": args.platform, "roles": 8,
        "originalInstallerSha256": suite.digest(package),
    })
    print(f"Verified original native payloads: 8/8 for {args.platform}/amd64", flush=True)


if __name__ == "__main__":
    try:
        main()
    except (suite.SuiteError, OSError, ValueError, KeyError, subprocess.CalledProcessError) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        sys.exit(1)
