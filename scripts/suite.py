#!/usr/bin/env python3
"""Build, collect and verify the complete SimLab suite. Python 3.10+; stdlib only."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform as host_platform
import plistlib
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile

ROOT = Path(__file__).resolve().parents[1]
EXPECTED_IDS = {
    "iec104.master", "iec104.slave", "modbus.master", "modbus.slave",
    "pmu.master", "pmu.sub", "opcua.master", "opcua.server",
}


class SuiteError(RuntimeError):
    pass


def run(args, cwd=None, env=None, capture=False):
    args = list(map(str, args))
    if os.name == "nt" and args[0] == "npm":
        npm = Path(require("npm"))
        cli = npm.parent / "node_modules/npm/bin/npm-cli.js"
        if not cli.is_file():
            raise SuiteError(f"Cannot resolve the npm Node entry point: {cli}")
        args = [require("node"), str(cli)] + args[1:]
    print("+ " + " ".join(map(str, args)), flush=True)
    result = subprocess.run(args, cwd=cwd, env=env, check=True,
                            text=True, stdout=subprocess.PIPE if capture else None)
    return result.stdout.strip() if capture else None


def require(command):
    path = shutil.which(command)
    if not path:
        raise SuiteError(f"Required tool is missing: {command}")
    return path


def git_bash(git_executable):
    git_directory = Path(git_executable).resolve().parent
    for candidate in (
        git_directory / "bash.exe",
        git_directory.parent / "bin/bash.exe",
        git_directory.parent / "usr/bin/bash.exe",
        git_directory.parent.parent / "bin/bash.exe",
    ):
        if candidate.is_file():
            return str(candidate)
    raise SuiteError("Git for Windows Bash was not found; the WSL bash shim is not a supported build shell")


def bash():
    return git_bash(require("git")) if os.name == "nt" else require("bash")


def digest(path):
    value = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            value.update(block)
    return value.hexdigest()


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def tree_snapshot(root):
    result = {}
    for path in sorted(root.rglob("*")):
        relative = path.relative_to(root).as_posix()
        if path.is_symlink():
            target = os.readlink(path)
            result[relative] = {"type": "link", "sha256": hashlib.sha256(os.fsencode(target)).hexdigest()}
        elif path.is_file():
            result[relative] = {"type": "file", "sha256": digest(path)}
    return result


def tree_digest(snapshot):
    return hashlib.sha256(json.dumps(snapshot, sort_keys=True).encode()).hexdigest()


def launcher_source_fingerprint(root):
    files = [root / name for name in ("main.go", "app.go", "go.mod", "go.sum", "wails.json")]
    for directory in (root / "internal", root / "assets", root / "frontend/src", root / "build/darwin"):
        if directory.is_dir():
            files += [path for path in directory.rglob("*") if path.is_file()]
    files += [path for path in (root / "frontend").glob("*") if path.is_file() and path.suffix in (".json", ".ts", ".html")]
    value = hashlib.sha256()
    for path in sorted(set(files)):
        value.update(path.relative_to(root).as_posix().encode())
        value.update(path.read_bytes())
    return value.hexdigest()


def read_lock(root=ROOT):
    value = json.loads((root / "components.lock.json").read_text(encoding="utf-8"))
    ids = [item["id"] for item in value["components"]]
    if value.get("schemaVersion") != 1 or set(ids) != EXPECTED_IDS or len(ids) != 8:
        raise SuiteError("The lock must contain exactly the eight supported component IDs")
    if len(value["repositories"]) != 4:
        raise SuiteError("The lock must contain four source repositories")
    for repo in value["repositories"]:
        if not re.fullmatch(r"[0-9a-f]{40}", repo["commit"]):
            raise SuiteError(f"Invalid source commit: {repo['id']}")
        if repo.get("cargoLock") and digest(root / repo["cargoLock"]) != repo["cargoLockSha256"]:
            raise SuiteError(f"Cargo dependency lock checksum mismatch: {repo['id']}")
    return value


def native_target(target, arch):
    native_os = {"Darwin": "darwin", "Windows": "windows", "Linux": "linux"}.get(host_platform.system())
    native_arch = {"x86_64": "amd64", "AMD64": "amd64", "arm64": "arm64", "aarch64": "arm64"}.get(host_platform.machine())
    if (target, arch) != (native_os, native_arch):
        raise SuiteError(f"Use a native {target}/{arch} builder; this host is {native_os}/{native_arch}")


def git(repo, *args, capture=True):
    return run(["git", "-C", repo, *args], capture=capture)


def fingerprint(repo):
    status = git(repo, "status", "--porcelain=v1", "--untracked-files=all")
    value = hashlib.sha256(status.encode())
    # Include working-tree content as well as the index. Ignored build outputs
    # and dependency directories deliberately do not participate.
    names = subprocess.check_output(["git", "-C", str(repo), "ls-files", "--modified", "--others", "--exclude-standard", "-z"])
    for name in sorted(set(names.split(b"\0")) - {b""}):
        path = repo / os.fsdecode(name)
        value.update(name)
        if path.is_symlink():
            value.update(os.fsencode(os.readlink(path)))
        elif path.is_file():
            value.update(path.read_bytes())
        else:
            value.update(b"deleted")
    value.update(subprocess.check_output(["git", "-C", str(repo), "diff", "--cached", "--binary"]))
    return value.hexdigest()


def state_path(workspace, repo):
    return workspace / ".state" / (repo["id"] + ".json")


def record_state(workspace, repo, root=ROOT):
    path = workspace / repo["directory"]
    write_json(state_path(workspace, repo), {
        "commit": repo["commit"], "patchSha256": digest(root / repo["patch"]),
        "fingerprint": fingerprint(path),
    })


def assert_source(workspace, repo, root=ROOT, allow_adopt=False):
    path = workspace / repo["directory"]
    if git(path, "rev-parse", "HEAD") != repo["commit"]:
        raise SuiteError(f"Source commit mismatch: {path}")
    stamp = state_path(workspace, repo)
    if stamp.exists():
        state = json.loads(stamp.read_text())
        if state != {"commit": repo["commit"], "patchSha256": digest(root / repo["patch"]), "fingerprint": fingerprint(path)}:
            raise SuiteError(f"Source was changed outside the recorded build: {path}; preserve it and use a fresh workspace")
    elif allow_adopt:
        # This explicit path accepts the initial agent-produced build only if
        # its entire tracked diff is the reviewed patch, including new files.
        actual = subprocess.check_output(["git", "-C", str(path), "diff", "HEAD", "--binary", "--no-ext-diff"])
        if actual != (root / repo["patch"]).read_bytes():
            raise SuiteError(f"Existing source does not exactly match its suite patch: {path}")
    else:
        raise SuiteError(f"No bootstrap state for {path}; bootstrap a clean clone first")


def bootstrap(workspace, root=ROOT):
    lock = read_lock(root)
    require("git")
    workspace.mkdir(parents=True, exist_ok=True)
    for repo in lock["repositories"]:
        path = workspace / repo["directory"]
        if state_path(workspace, repo).exists():
            assert_source(workspace, repo, root)
            continue
        if path.exists():
            # Never reset, clean, pull or overwrite an unknown dirty checkout.
            if git(path, "status", "--porcelain=v1", "--untracked-files=all"):
                raise SuiteError(f"Refusing to adapt a dirty checkout: {path}")
            if git(path, "rev-parse", "HEAD") != repo["commit"]:
                raise SuiteError(f"Clean checkout has a different commit: {path}; use a fresh workspace")
        else:
            run(["git", "init", path])
            git(path, "config", "core.autocrlf", "false", capture=False)
            git(path, "remote", "add", "origin", repo["url"], capture=False)
            git(path, "fetch", "--depth=1", "origin", repo["commit"], capture=False)
            git(path, "checkout", "--detach", "FETCH_HEAD", capture=False)
        patch = root / repo["patch"]
        git(path, "apply", "--check", "--index", patch, capture=False)
        git(path, "apply", "--index", patch, capture=False)
        record_state(workspace, repo, root)


def wails():
    binary = shutil.which("wails")
    if not binary:
        binary = str(Path(run(["go", "env", "GOPATH"], capture=True).split(os.pathsep)[0]) / "bin" / ("wails.exe" if os.name == "nt" else "wails"))
    if not Path(binary).is_file() or run([binary, "version"], capture=True).splitlines()[0] != "v2.14.0":
        raise SuiteError("Install the pinned CLI: go install github.com/wailsapp/wails/v2/cmd/wails@v2.14.0")
    return binary


def build(workspace, target, arch, root=ROOT):
    native_target(target, arch)
    lock = read_lock(root)
    for tool in ("git", "npm", "go", "cargo", "rustc"):
        require(tool)
    wails_binary = wails()
    env = os.environ.copy()
    env["VITE_SIMLAB_SUITE"] = "1"
    if target == "darwin":
        env["CGO_ENABLED"] = "1"
        env["CGO_CFLAGS"] = env.get("CGO_CFLAGS", "") + " -mmacosx-version-min=13.0"
        env["CGO_LDFLAGS"] = env.get("CGO_LDFLAGS", "") + " -framework UniformTypeIdentifiers -mmacosx-version-min=13.0"
        env["MACOSX_DEPLOYMENT_TARGET"] = "13.0"
    elif target == "windows":
        env["CGO_ENABLED"] = "0"
        triple = "aarch64-pc-windows-msvc" if arch == "arm64" else "x86_64-pc-windows-msvc"
        env["CARGO_TARGET_" + triple.upper().replace("-", "_") + "_RUSTFLAGS"] = "-C target-feature=+crt-static"
    for repo in lock["repositories"]:
        assert_source(workspace, repo, root)
        source = workspace / repo["directory"]
        components = [item for item in lock["components"] if item["repository"] == repo["id"]]
        if repo.get("cargoLock"):
            cargo_lock = source / "Cargo.lock"
            if cargo_lock.exists() and digest(cargo_lock) != repo["cargoLockSha256"]:
                raise SuiteError(f"Dependency lock differs: {cargo_lock}")
            shutil.copy2(root / repo["cargoLock"], cargo_lock)
        if repo["id"] == "modbus":
            run(["npm", "ci", "--ignore-scripts", "--no-audit", "--no-fund"], cwd=source, env=env)
        for component in components:
            if repo["id"] == "modbus":
                args = [bash(), (source / "go/scripts/suite.sh").as_posix(), component["id"].split(".")[1], "-platform", f"{target}/{arch}", "-trimpath"]
                if target == "windows":
                    args += ["-webview2", "error"]
                run(args, cwd=source, env=env)
            else:
                frontend = source / component["frontend"]
                run(["npm", "ci", "--no-audit", "--no-fund"], cwd=frontend, env=env)
                run(["npm", "run", "build"], cwd=frontend, env=env)
                cli = frontend / "node_modules/@tauri-apps/cli/tauri.js"
                args = [require("node"), cli, "build", "--ci", "--features", "simlab-suite", "--config", "tauri.suite.conf.json"]
                args += ["--bundles", "app"] if target == "darwin" else ["--no-bundle"]
                if target == "windows":
                    args += ["--target", triple]
                run(args + ["--", "--locked"], cwd=source / component["project"], env=env)
            source_exe = payload_executable(source, component, target, arch)
            write_json(workspace / ".state" / (component["id"] + ".build.json"), {
                "commit": repo["commit"], "patchSha256": digest(root / repo["patch"]),
                "platform": target, "architecture": arch, "sha256": digest(source_exe), "suiteBuild": True,
                "payloadTreeSha256": source_payload_digest(source, component, target, arch),
            })
        # Record only changes made by the explicit tool invocations above;
        # subsequent unrecorded source edits cause the next build to fail.
        record_state(workspace, repo, root)
    run(["npm", "ci", "--no-audit", "--no-fund"], cwd=root / "frontend", env=env)
    args = [wails_binary, "build", "-platform", f"{target}/{arch}", "-trimpath"]
    if target == "linux":
        args += ["-tags", "webkit2_41"]
    if target == "windows":
        args += ["-webview2", "error"]
    run(args, cwd=root, env=env)
    write_json(workspace / ".state/launcher.build.json", launcher_receipt(root, lock, target, arch))


def app_entry(app):
    try:
        raw = (app / "Contents/Info.plist").read_bytes()
        # Wails emits valid XML beginning with a DOCTYPE, without <?xml>.
        info = plistlib.loads(raw, fmt=None if raw.startswith(b"bplist") else plistlib.FMT_XML)
        binary = info["CFBundleExecutable"]
    except (OSError, KeyError, plistlib.InvalidFileException) as error:
        raise SuiteError(f"Invalid macOS application: {app}: {error}") from error
    if not isinstance(binary, str) or binary in (".", "..") or Path(binary).name != binary:
        raise SuiteError(f"Invalid CFBundleExecutable: {app}")
    entry = app / "Contents/MacOS" / binary
    if not entry.is_file():
        raise SuiteError(f"Missing application executable: {entry}")
    return entry, info


def payload_executable(source, component, target, arch=None):
    if target == "darwin":
        app = source / (component["project"] + "/build/bin" if component["repository"] == "modbus" else "target/release/bundle/macos") / (component["product"] + ".app")
        return app_entry(app)[0]
    folder = source / (component["project"] + "/build/bin" if component["repository"] == "modbus" else "target/release")
    if target == "windows" and arch and component["repository"] != "modbus":
        triple = "aarch64-pc-windows-msvc" if arch == "arm64" else "x86_64-pc-windows-msvc"
        folder = source / "target" / triple / "release"
    return folder / (component["binary"] + (".exe" if target == "windows" else ""))


def safe_path(root, relative):
    path = Path(relative)
    if not relative or path.is_absolute() or ".." in path.parts or "\\" in relative:
        raise SuiteError(f"Unsafe manifest path: {relative}")
    result = (root / path).resolve()
    if not result.is_relative_to(root.resolve()):
        raise SuiteError(f"Path escapes the suite: {relative}")
    return result


def assert_tree_links(root):
    for path in root.rglob("*"):
        if path.is_symlink() and not path.resolve().is_relative_to(root.resolve()):
            raise SuiteError(f"Payload symlink escapes the suite: {path}")


def sign_app(app, identity="-"):
    # Sign nested Mach-O code and bundles from the inside out. Never use
    # --deep for signing: it can overwrite child entitlements indiscriminately.
    paths = []
    for path in app.rglob("*"):
        if path.is_symlink():
            continue
        if path.is_dir() and path.suffix in (".app", ".framework", ".xpc", ".bundle"):
            paths.append(path)
        elif path.is_file():
            with path.open("rb") as stream:
                if stream.read(4) in (b"\xcf\xfa\xed\xfe", b"\xfe\xed\xfa\xcf", b"\xca\xfe\xba\xbe", b"\xbe\xba\xfe\xca"):
                    paths.append(path)
    for path in sorted(paths, key=lambda item: len(item.parts), reverse=True) + [app]:
        run(["codesign", "--force", "--sign", identity, path])
    run(["codesign", "--verify", "--deep", "--strict", app])


def copy_portable(source, component, target, destination, arch=None):
    entry = payload_executable(source, component, target, arch)
    if not entry.is_file():
        raise SuiteError(f"Missing required payload: {entry}")
    destination.mkdir(parents=True)
    shutil.copy2(entry, destination / entry.name)
    # Tauri currently embeds its frontends, but collect declared resources and
    # neighboring DLLs too; unsupported resource declarations fail explicitly.
    if component["repository"] != "modbus":
        config = json.loads((source / component["project"] / "tauri.conf.json").read_text())
        bundle = config.get("bundle", {})
        resources = bundle.get("resources", []) or []
        mappings = resources.items() if isinstance(resources, dict) else [(name, name) for name in resources]
        for original, relative in mappings:
            original_path = source / component["project"] / original
            output = safe_path(destination, relative)
            if not original_path.exists():
                raise SuiteError(f"Declared resource is missing or uses an unsupported glob: {original_path}")
            output.parent.mkdir(parents=True, exist_ok=True)
            if original_path.is_dir():
                shutil.copytree(original_path, output, symlinks=True)
            else:
                shutil.copy2(original_path, output)
        if bundle.get("externalBin"):
            raise SuiteError(f"externalBin needs an explicit portable mapping: {component['id']}")
    else:
        for path in entry.parent.iterdir():
            if path != entry and path.suffix.lower() not in (".app", ".exe", ".msi", ".pdb"):
                if path.is_dir():
                    shutil.copytree(path, destination / path.name, symlinks=True)
                else:
                    shutil.copy2(path, destination / path.name)
    for dll in entry.parent.glob("*.dll"):
        shutil.copy2(dll, destination / dll.name)
    return destination / entry.name


def source_payload_digest(source, component, target, arch):
    if target == "darwin":
        return tree_digest(tree_snapshot(payload_executable(source, component, target, arch).parents[2]))
    with tempfile.TemporaryDirectory(prefix="simlab-payload-") as directory:
        output = Path(directory) / "payload"
        copy_portable(source, component, target, output, arch)
        return tree_digest(tree_snapshot(output))


def copy_launcher(root, target, destination):
    source = root / "build/bin"
    name = "SimLab.exe" if target == "windows" else "SimLab"
    if not (source / name).is_file():
        raise SuiteError(f"Missing launcher: {source / name}")
    destination.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source / name, destination / name)
    for path in source.iterdir():
        if path.name == name or path.suffix.lower() in (".app", ".exe", ".msi", ".pdb"):
            continue
        if path.is_dir():
            shutil.copytree(path, destination / path.name, symlinks=True)
        elif path.suffix.lower() == ".dll":
            shutil.copy2(path, destination / path.name)
    return destination / name


def launcher_receipt(root, lock, target, arch):
    config = json.loads((root / "wails.json").read_text())
    if config["info"]["productVersion"] != lock["suiteVersion"]:
        raise SuiteError("Launcher source version does not match the suite lock")
    if target == "darwin":
        app = root / "build/bin/SimLab.app"
        entry, _ = app_entry(app)
        snapshot = tree_snapshot(app)
    else:
        with tempfile.TemporaryDirectory(prefix="simlab-launcher-") as directory:
            entry = copy_launcher(root, target, Path(directory))
            snapshot = tree_snapshot(Path(directory))
    entry = root / "build/bin/SimLab.app/Contents/MacOS/SimLab" if target == "darwin" else root / "build/bin" / ("SimLab.exe" if target == "windows" else "SimLab")
    return {"suiteVersion": lock["suiteVersion"], "platform": target, "architecture": arch,
            "sourceFingerprint": launcher_source_fingerprint(root), "sha256": digest(entry),
            "payloadTreeSha256": tree_digest(snapshot)}


def assert_architecture(executable, target, arch):
    if target == "darwin":
        # -verify_arch consumes all following arguments as architecture names.
        run(["lipo", executable, "-verify_arch", "arm64" if arch == "arm64" else "x86_64"])
    elif target == "windows":
        with executable.open("rb") as stream:
            if stream.read(2) != b"MZ":
                raise SuiteError(f"Not a Windows PE executable: {executable}")
            stream.seek(0x3C)
            offset = int.from_bytes(stream.read(4), "little")
            stream.seek(offset)
            header = stream.read(6)
        if header[:4] != b"PE\0\0" or int.from_bytes(header[4:], "little") != {"amd64": 0x8664, "arm64": 0xAA64}[arch]:
            raise SuiteError(f"Windows architecture mismatch: {executable}")
    else:
        with executable.open("rb") as stream:
            header = stream.read(20)
        if header[:4] != b"\x7fELF" or int.from_bytes(header[18:20], "little") != {"amd64": 62, "arm64": 183}[arch]:
            raise SuiteError(f"Linux architecture mismatch: {executable}")


def collect(workspace, target, arch, root=ROOT, adopt=False, identity="-"):
    lock = read_lock(root)
    repos = {item["id"]: item for item in lock["repositories"]}
    for repo in repos.values():
        assert_source(workspace, repo, root, allow_adopt=adopt)
    if not (adopt and target == "darwin"):
        stamp = workspace / ".state/launcher.build.json"
        if not stamp.is_file() or json.loads(stamp.read_text()) != launcher_receipt(root, lock, target, arch):
            raise SuiteError("Missing or stale launcher source/version/payload receipt")
    output = root / "dist/staging" / f"{target}-{arch}"
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".collect-", dir=output.parent) as directory:
        temporary = Path(directory)
        if target == "darwin":
            source = root / "build/bin/SimLab.app"
            root_entry, info = app_entry(source)
            if info.get("CFBundleShortVersionString") != lock["suiteVersion"]:
                raise SuiteError("Launcher version does not match components.lock.json")
            if tuple(map(int, info.get("LSMinimumSystemVersion", "0").split("."))) < (13, 0):
                raise SuiteError("The suite launcher must declare macOS 13.0 or later")
            assert_architecture(root_entry, target, arch)
            app = temporary / "SimLab.app"
            shutil.copytree(source, app, symlinks=True)
            suite_root = app / "Contents"
            (suite_root / "Helpers").mkdir(exist_ok=True)
        else:
            suite_root = temporary
            source = root / "build/bin" / ("SimLab.exe" if target == "windows" else "SimLab")
            if not source.is_file():
                raise SuiteError(f"Missing launcher: {source}")
            assert_architecture(source, target, arch)
            copy_launcher(root, target, suite_root)
        launcher_files = tree_snapshot(suite_root) if target != "darwin" else None
        entries = []
        for component in lock["components"]:
            repo = repos[component["repository"]]
            source = workspace / repo["directory"]
            original = payload_executable(source, component, target, arch)
            if not original.is_file():
                raise SuiteError(f"Missing required role {component['id']}: {original}")
            assert_architecture(original, target, arch)
            receipt = workspace / ".state" / (component["id"] + ".build.json")
            if not (adopt and target == "darwin"):
                expected = {"commit": repo["commit"], "patchSha256": digest(root / repo["patch"]), "platform": target, "architecture": arch, "sha256": digest(original), "suiteBuild": True, "payloadTreeSha256": source_payload_digest(source, component, target, arch)}
                if not receipt.is_file() or json.loads(receipt.read_text()) != expected:
                    raise SuiteError(f"Missing or stale suite-build receipt: {component['id']}")
            if target == "darwin":
                source_app = original.parents[2]
                _, info = app_entry(source_app)
                if info.get("CFBundleIdentifier") != component["bundleId"] or info.get("CFBundleShortVersionString") != component["version"]:
                    raise SuiteError(f"Child bundle identity/version mismatch: {component['id']}")
                child = suite_root / "Helpers" / (component["product"] + ".app")
                shutil.copytree(source_app, child, symlinks=True)
                sign_app(child, identity)
                executable, _ = app_entry(child)
                payload_root = child
            else:
                executable = copy_portable(source, component, target, suite_root / "apps" / component["id"], arch)
                payload_root = executable.parent
            entries.append({"id": component["id"], "version": component["version"], "sourceCommit": repo["commit"], "executable": executable.relative_to(suite_root).as_posix(), "sha256": digest(executable), "suiteBuild": True,
                            "payloadRoot": payload_root.relative_to(suite_root).as_posix(), "payloadFiles": tree_snapshot(payload_root)})
        assert_tree_links(suite_root)
        manifest = suite_root / ("Resources/suite-manifest.json" if target == "darwin" else "suite-manifest.json")
        manifest_value = {"schemaVersion": 1, "suiteVersion": lock["suiteVersion"], "platform": target, "architecture": arch, "components": entries}
        if launcher_files is not None:
            manifest_value["launcherFiles"] = launcher_files
        write_json(manifest, manifest_value)
        if target == "darwin":
            # Child hashes are final; only sign the outer app now.
            run(["codesign", "--force", "--sign", identity, app])
            run(["codesign", "--verify", "--deep", "--strict", app])
        verify_manifest(suite_root, lock)
        previous = output.with_name(".previous-" + temporary.name)
        if output.exists():
            output.rename(previous)
        try:
            temporary.rename(output)
        except OSError:
            if previous.exists():
                previous.rename(output)
            raise
        if previous.exists():
            shutil.rmtree(previous)
    print(f"Complete {target}/{arch} stage: {output}")
    return output


def verify_manifest(suite_root, lock=None):
    manifest_path = suite_root / ("Resources/suite-manifest.json" if suite_root.name == "Contents" else "suite-manifest.json")
    value = json.loads(manifest_path.read_text())
    entries = value.get("components", [])
    if value.get("schemaVersion") != 1 or len(entries) != 8 or {item["id"] for item in entries} != EXPECTED_IDS:
        raise SuiteError("Manifest is incomplete or contains duplicate/unknown roles")
    expected = {item["id"]: item for item in lock["components"]} if lock else {}
    repos = {item["id"]: item for item in lock["repositories"]} if lock else {}
    if value.get("platform") not in ("darwin", "windows", "linux") or value.get("architecture") not in ("amd64", "arm64"):
        raise SuiteError("Unsupported manifest target")
    if lock and value.get("suiteVersion") != lock["suiteVersion"]:
        raise SuiteError("Suite version mismatch")
    for relative, expected_file in value.get("launcherFiles", {}).items():
        file = safe_path(suite_root, relative)
        if not file.is_file() or digest(file) != expected_file["sha256"]:
            raise SuiteError(f"Launcher resource changed: {relative}")
    for item in entries:
        executable = safe_path(suite_root, item["executable"])
        if not item.get("suiteBuild") or not re.fullmatch(r"[0-9a-f]{40}", item.get("sourceCommit", "")):
            raise SuiteError(f"Missing suite build/source identity: {item['id']}")
        if not executable.is_file() or digest(executable) != item["sha256"]:
            raise SuiteError(f"Payload missing or checksum mismatch: {item['id']}")
        if "payloadFiles" in item and tree_snapshot(safe_path(suite_root, item["payloadRoot"])) != item["payloadFiles"]:
            raise SuiteError(f"Payload resource tree changed: {item['id']}")
        if lock:
            component = expected[item["id"]]
            if item["version"] != component["version"] or item["sourceCommit"] != repos[component["repository"]]["commit"]:
                raise SuiteError(f"Component version/source mismatch: {item['id']}")
            if value["platform"] == "darwin":
                _, info = app_entry(executable.parents[2])
                if info.get("CFBundleIdentifier") != component["bundleId"] or info.get("CFBundleShortVersionString") != component["version"]:
                    raise SuiteError(f"Component bundle identity mismatch: {item['id']}")
    assert_tree_links(suite_root)
    return value


def suite_root_for(root, target, arch):
    stage = root / "dist/staging" / f"{target}-{arch}"
    return stage / "SimLab.app/Contents" if target == "darwin" else stage


def package(target, arch, root=ROOT, webview2=None, webview2_sha256=None):
    lock = read_lock(root)
    stage = root / "dist/staging" / f"{target}-{arch}"
    suite_root = suite_root_for(root, target, arch)
    verify_manifest(suite_root, lock)
    output = root / "dist/packages"
    output.mkdir(parents=True, exist_ok=True)
    name = f"SimLab_{lock['suiteVersion']}_{target}-{arch}"
    if target == "darwin":
        require("ditto")
        require("hdiutil")
        run(["codesign", "--verify", "--deep", "--strict", stage / "SimLab.app"])
        zip_path = output / (name + ".zip")
        run(["ditto", "-c", "-k", "--sequesterRsrc", "--keepParent", stage / "SimLab.app", zip_path])
        write_json(zip_path.with_suffix(".zip.sha256.json"), {"file": zip_path.name, "sha256": digest(zip_path)})
        destination = output / (name + ".dmg")
        with tempfile.TemporaryDirectory(prefix=".dmg-", dir=output) as directory:
            image_root = Path(directory)
            shutil.copytree(stage / "SimLab.app", image_root / "SimLab.app", symlinks=True)
            (image_root / "Applications").symlink_to("/Applications")
            run(["hdiutil", "create", "-ov", "-format", "UDZO", "-volname", "SimLab", "-srcfolder", image_root, destination])
        run(["hdiutil", "verify", destination])
    elif target == "windows":
        makensis = require("makensis")
        if webview2 is None and webview2_sha256 is None:
            runtime_lock = json.loads((root / "packaging/windows/webview2.lock.json").read_text())
            if runtime_lock["architecture"] != arch:
                raise SuiteError("No default offline WebView2 runtime is locked for this architecture")
            webview2 = root / ".tools/WebView2Runtime.exe"
            webview2_sha256 = runtime_lock["sha256"]
            if not webview2.is_file():
                run([sys.executable, root / "scripts/fetch.py", runtime_lock["url"], webview2_sha256, webview2])
        if not webview2 or not webview2_sha256 or not re.fullmatch(r"[0-9a-f]{64}", webview2_sha256):
            raise SuiteError("Windows packaging requires a pinned offline WebView2 installer and its SHA256")
        if digest(webview2) != webview2_sha256:
            raise SuiteError("Offline WebView2 installer checksum mismatch")
        destination = output / (name + "-setup.exe")
        if Path(webview2).stat().st_size < 20 * 1024 * 1024:
            raise SuiteError("WebView2 payload is too small for an offline standalone runtime installer")
        run([makensis, f"/DARCH={arch}", f"/DVERSION={lock['suiteVersion']}", f"/DSTAGE={stage}", f"/DWEBVIEW2={Path(webview2).resolve()}", f"/DCHECKSCRIPT={root / 'packaging/windows/check-payload.ps1'}", f"/DOUTPUT={destination}", root / "packaging/windows/simlab.nsi"])
    else:
        native_target(target, arch)
        require("ldd")
        runtime_script = root / "packaging/linux/runtime.py"
        run([sys.executable, runtime_script, "audit", stage])
        destination = output / (name + ".run")
        index = stage / ".files.sha256"
        indexed = []
        for path in sorted(stage.rglob("*")):
            if path.is_file() and path != index and path.name != ".complete":
                relative = path.relative_to(stage).as_posix()
                if any(char in relative for char in ("\n", "\r", "\\")):
                    raise SuiteError(f"Filename cannot be indexed safely: {relative}")
                indexed.append(f"{digest(path)}  {relative}\n")
        index.write_text("".join(indexed))
        with tempfile.NamedTemporaryFile(suffix=".tar.gz") as temporary:
            with tarfile.open(temporary.name, "w:gz") as archive:
                archive.add(stage, arcname="SimLab", recursive=True)
            payload_hash = digest(temporary.name)
            header = (root / "packaging/linux/self-extract.sh").read_text().replace("@PAYLOAD_SHA256@", payload_hash).replace("@INDEX_SHA256@", digest(index)).replace("@VERSION@", lock["suiteVersion"]).replace("@ARCH@", arch)
            with destination.open("wb") as stream:
                stream.write(header.encode())
                stream.write(Path(temporary.name).read_bytes())
        destination.chmod(0o755)
    write_json(destination.with_suffix(destination.suffix + ".sha256.json"), {"file": destination.name, "sha256": digest(destination)})
    print(f"Package: {destination}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["bootstrap", "build", "collect", "verify", "package"])
    parser.add_argument("--workspace", type=Path, default=ROOT / ".components")
    parser.add_argument("--platform", choices=["darwin", "windows", "linux"], default={"Darwin": "darwin", "Windows": "windows", "Linux": "linux"}.get(host_platform.system()))
    parser.add_argument("--arch", choices=["arm64", "amd64"], default="arm64" if host_platform.machine().lower() in ("arm64", "aarch64") else "amd64")
    parser.add_argument("--adopt-existing", action="store_true", help="Explicitly accept this initial macOS build after exact patch and bundle-ID checks")
    parser.add_argument("--signing-identity", default=os.environ.get("SIMLAB_SIGNING_IDENTITY", "-"))
    parser.add_argument("--webview2", type=Path)
    parser.add_argument("--webview2-sha256")
    args = parser.parse_args()
    if args.command == "bootstrap":
        bootstrap(args.workspace.resolve())
    elif args.command == "build":
        build(args.workspace.resolve(), args.platform, args.arch)
    elif args.command == "collect":
        collect(args.workspace.resolve(), args.platform, args.arch, adopt=args.adopt_existing, identity=args.signing_identity)
    elif args.command == "verify":
        value = verify_manifest(suite_root_for(ROOT, args.platform, args.arch), read_lock())
        print(f"Verified {len(value['components'])}/8 roles for {args.platform}/{args.arch}")
    else:
        package(args.platform, args.arch, webview2=args.webview2, webview2_sha256=args.webview2_sha256)


if __name__ == "__main__":
    try:
        main()
    except (SuiteError, OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        sys.exit(1)
