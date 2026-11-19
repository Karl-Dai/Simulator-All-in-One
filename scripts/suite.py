#!/usr/bin/env python3
"""Build, collect and verify the complete Simulator All-in-One suite. Python 3.10+; stdlib only."""
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

sys.path.insert(0, str(Path(__file__).resolve().parent))
import toolchain
import windows_portable

ROOT = Path(__file__).resolve().parents[1]
BUILD_TOOLS = ("node", "npm", "python", "go", "rust", "wails")
EXPECTED_IDS = {
    "iec104.master", "iec104.slave", "modbus.master", "modbus.slave",
    "pmu.master", "pmu.sub", "opcua.master", "opcua.server",
    "iec101.master", "iec101.slave", "iec102.master", "iec102.slave",
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
                            text=True, encoding="utf-8", stdout=subprocess.PIPE if capture else None)
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
    files = [root / name for name in ("main.go", "app.go", "go.mod", "go.sum", "wails.json", "toolchain.lock.json")]
    for directory in (root / "internal", root / "assets", root / "frontend/src", root / "ui", root / "build/darwin"):
        if directory.is_dir():
            files += [path for path in directory.rglob("*") if path.is_file()]
    files += [path for path in (root / "frontend").glob("*") if path.is_file() and path.suffix in (".json", ".ts", ".html")]
    value = hashlib.sha256()
    for path in sorted(set(files)):
        value.update(path.relative_to(root).as_posix().encode())
        value.update(path.read_bytes())
    return value.hexdigest()


def read_manifest(root=ROOT):
    value = json.loads((root / "components.json").read_text(encoding="utf-8"))
    config = json.loads((root / "wails.json").read_text(encoding="utf-8"))
    value["suiteVersion"] = config["info"]["productVersion"]
    if not re.fullmatch(r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)", value["suiteVersion"]):
        raise SuiteError("The suite version must be a three-part release version")
    ids = [item["id"] for item in value["components"]]
    if value.get("schemaVersion") != 2 or set(ids) != EXPECTED_IDS or len(ids) != 12:
        raise SuiteError("The manifest must contain exactly the twelve supported component IDs")
    for component in value["components"]:
        directory = root / "components" / component["repository"]
        if not directory.is_dir():
            raise SuiteError(f"Missing component source directory: {directory}")
    return value


def source_commit(root=ROOT):
    return git(root, "rev-parse", "HEAD").strip()


def assert_clean_worktree(root=ROOT):
    status = git(root, "status", "--porcelain", "--untracked-files=no")
    if status.strip():
        raise SuiteError("Suite builds require a clean git worktree; commit or stash tracked changes first:\n" + status.strip())


def inject_icons(root=ROOT):
    """Copy role icons from assets/roles into the component trees before building."""
    manifest = read_manifest(root)
    sizes = ("32x32.png", "128x128.png", "128x128@2x.png", "icon.png", "icon.icns", "icon.ico")
    for component in manifest["components"]:
        source = root / "assets/roles" / component["id"]
        repo = root / "components" / component["repository"]
        targets = [(source / "icon.svg", repo / component["frontend"] / "public/suite-icon.svg")]
        targets += [(source / name, repo / component["project"] / "suite-icons" / name) for name in sizes]
        if component["repository"] == "modbus":
            targets.append((source / "128x128.png", repo / "go/internal/suitehost/icons" / (component["id"] + ".png")))
        for origin, destination in targets:
            if not origin.is_file():
                raise SuiteError(f"Missing role icon: {origin}")
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(origin, destination)


def native_target(target, arch):
    native_os = {"Darwin": "darwin", "Windows": "windows", "Linux": "linux"}.get(host_platform.system())
    native_arch = {"x86_64": "amd64", "AMD64": "amd64", "arm64": "arm64", "aarch64": "arm64"}.get(host_platform.machine())
    if (target, arch) != (native_os, native_arch):
        raise SuiteError(f"Use a native {target}/{arch} builder; this host is {native_os}/{native_arch}")


def git(repo, *args, capture=True):
    return run(["git", "-C", repo, *args], capture=capture)


def wails(root=ROOT):
    binary = shutil.which("wails")
    if not binary:
        binary = str(Path(run(["go", "env", "GOPATH"], capture=True).split(os.pathsep)[0]) / "bin" / ("wails.exe" if os.name == "nt" else "wails"))
    version = toolchain.load_lock(root / "toolchain.lock.json")["tools"]["wails"]["version"]
    if not Path(binary).is_file() or run([binary, "version"], capture=True).splitlines()[0] != "v" + version:
        raise SuiteError(f"Install the pinned CLI: go install github.com/wailsapp/wails/v2/cmd/wails@v{version}")
    return binary


def build_tool_names(target):
    return BUILD_TOOLS


def native_version_probe(argv, first_line=False):
    """Capture bounded diagnostic version output, without changing the host."""
    result = {"command": list(argv)}
    executable = shutil.which(argv[0])
    if executable is None:
        result["error"] = f"{argv[0]} is not installed or not on PATH"
        return result
    # Keep proxy entry names such as cargo -> rustup intact for argv[0] dispatch.
    result["path"] = executable
    try:
        process = subprocess.run(
            [executable, *argv[1:]], shell=False, stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
            encoding="utf-8", errors="replace", timeout=10, check=False,
        )
    except subprocess.TimeoutExpired:
        result["error"] = f"{argv[0]} timed out after 10 seconds"
        return result
    except OSError as error:
        result["error"] = f"Cannot run {argv[0]}: {error}"
        return result
    result["returncode"] = process.returncode
    # MSVC's compiler version banner is written to stderr, before stdout help.
    streams = (process.stderr, process.stdout) if argv[0] == "cl" else (process.stdout, process.stderr)
    lines = [line.strip() for output in streams for line in output.splitlines() if line.strip()]
    result["value"] = "\n".join(lines[:1] if first_line else lines[:6])[:1500]
    if process.returncode:
        result["error"] = f"{argv[0]} exited with status {process.returncode}"
    elif not lines:
        result["error"] = f"{argv[0]} returned no version output"
    return result


def inspect_native_environment(target):
    """Record optional native versions; missing diagnostics do not block builds."""
    record = {"os": host_platform.system(), "release": host_platform.release(),
              "machine": host_platform.machine(), "target": target,
              "cargo": native_version_probe(["cargo", "--version"], first_line=True)}
    if target == "darwin":
        record["sdk"] = native_version_probe(["xcrun", "--show-sdk-version"], first_line=True)
        record["xcode"] = native_version_probe(["xcodebuild", "-version"])
        record["clang"] = native_version_probe(["clang", "--version"], first_line=True)
    elif target == "linux":
        record["gtk"] = native_version_probe(["pkg-config", "--modversion", "gtk+-3.0"], first_line=True)
        record["webkitgtk"] = native_version_probe(["pkg-config", "--modversion", "webkit2gtk-4.1"], first_line=True)
    elif target == "windows":
        record["msvc"] = native_version_probe(["cl", "/?"], first_line=True)
        for field, variable in (("sdk", "WindowsSDKVersion"), ("ucrt", "UCRTVersion")):
            version = os.environ.get(variable, "").strip().rstrip("\\/")
            record[field] = {"source": variable}
            if version and re.fullmatch(r"[0-9]+(?:\.[0-9]+)+", version):
                record[field]["value"] = version
            else:
                record[field]["error"] = f"{variable} is missing or is not a numeric SDK version"
    return record


def inspect_build_toolchain(root, target, arch):
    path = root / "toolchain.lock.json"
    try:
        locked = toolchain.load_lock(path)
        observed = toolchain.check_tools(locked, names=build_tool_names(target), platform=target)
        return {"schemaVersion": 1, "lockSha256": toolchain.lock_digest(path),
                "platform": target, "architecture": arch, "tools": observed,
                "nativeEnvironment": inspect_native_environment(target)}
    except toolchain.ToolchainError as error:
        raise SuiteError(str(error)) from error


def validate_build_toolchain(record, root, target, arch):
    """Validate saved evidence without invoking or requiring installed compilers."""
    path = root / "toolchain.lock.json"
    try:
        locked = toolchain.load_lock(path)
        expected_digest = toolchain.lock_digest(path)
    except toolchain.ToolchainError as error:
        raise SuiteError(str(error)) from error
    if (not isinstance(record, dict) or record.get("schemaVersion") != 1
            or record.get("lockSha256") != expected_digest
            or record.get("platform") != target or record.get("architecture") != arch):
        raise SuiteError("Missing or stale build toolchain record; rebuild with the locked tools")
    observed = record.get("tools")
    if not isinstance(observed, list) or any(not isinstance(item, dict) for item in observed):
        raise SuiteError("Invalid build toolchain observations")
    names = [item.get("tool") for item in observed]
    if (any(not isinstance(name, str) for name in names)
            or len(names) != len(set(names)) or set(names) != set(build_tool_names(target))
            or any(name not in locked["tools"] for name in names)):
        raise SuiteError("Incomplete build toolchain observations")
    for item in observed:
        version = locked["tools"][item["tool"]]["version"]
        actual = item.get("actual")
        same_version = actual == version or (item["tool"] == "nsis" and isinstance(actual, str) and actual + ".0" == version)
        if (item.get("expected") != version or not same_version
                or item.get("matches") is not True or item.get("error")
                or not isinstance(item.get("path"), str) or not item["path"]):
            raise SuiteError(f"Invalid build toolchain observation: {item['tool']}")
    return record


def build_toolchain_digest(record):
    return hashlib.sha256(json.dumps(record, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def tauri_suite_config(source, component, version):
    """Override package_info and native metadata without changing standalone sources."""
    config = json.loads((source / component["project"] / "tauri.suite.conf.json").read_text(encoding="utf-8"))
    return json.dumps(dict(config, version=version))


def build(target, arch, root=ROOT):
    native_target(target, arch)
    assert_clean_worktree(root)
    tool_record = inspect_build_toolchain(root, target, arch)
    tool_digest = build_toolchain_digest(tool_record)
    manifest = read_manifest(root)
    inject_icons(root)
    for tool in ("git", "npm", "go", "cargo", "rustc"):
        require(tool)
    wails_binary = wails(root)
    head = source_commit(root)
    receipts = root / "dist/receipts"
    receipts.mkdir(parents=True, exist_ok=True)
    env = os.environ.copy()
    env["VITE_SIMLAB_SUITE"] = "1"
    env["SAIO_VERSION"] = manifest["suiteVersion"]
    if target == "darwin":
        env["CGO_ENABLED"] = "1"
        env["CGO_CFLAGS"] = env.get("CGO_CFLAGS", "") + " -mmacosx-version-min=13.0"
        env["CGO_LDFLAGS"] = env.get("CGO_LDFLAGS", "") + " -framework UniformTypeIdentifiers -mmacosx-version-min=13.0"
        env["MACOSX_DEPLOYMENT_TARGET"] = "13.0"
    elif target == "windows":
        env["CGO_ENABLED"] = "0"
        triple = "aarch64-pc-windows-msvc" if arch == "arm64" else "x86_64-pc-windows-msvc"
        env["CARGO_TARGET_" + triple.upper().replace("-", "_") + "_RUSTFLAGS"] = "-C target-feature=+crt-static"
    installed = set()
    for component in manifest["components"]:
        source = root / "components" / component["repository"]
        if component["repository"] == "modbus":
            if component["repository"] not in installed:
                run(["npm", "ci", "--ignore-scripts", "--no-audit", "--no-fund"], cwd=source, env=env)
                installed.add(component["repository"])
            args = [bash(), (source / "go/scripts/suite.sh").as_posix(), component["id"].split(".")[1], "-platform", f"{target}/{arch}", "-trimpath", "-m"]
            if target == "windows":
                args += ["-webview2", "error"]
            run(args, cwd=source, env=env)
        else:
            frontend = source / component["frontend"]
            run(["npm", "ci", "--no-audit", "--no-fund"], cwd=frontend, env=env)
            run(["npm", "run", "build"], cwd=frontend, env=env)
            cli = frontend / "node_modules/@tauri-apps/cli/tauri.js"
            args = [require("node"), cli, "build", "--ci", "--features", "simlab-suite", "--config", tauri_suite_config(source, component, manifest["suiteVersion"])]
            args += ["--bundles", "app"] if target == "darwin" else ["--no-bundle"]
            if target == "windows":
                args += ["--target", triple]
            run(args + ["--", "--locked"], cwd=source / component["project"], env=env)
        source_exe = payload_executable(source, component, target, arch)
        write_json(receipts / (component["id"] + ".build.json"), {
            "commit": head, "platform": target, "architecture": arch, "sha256": digest(source_exe), "suiteBuild": True,
            "suiteVersion": manifest["suiteVersion"],
            "payloadTreeSha256": source_payload_digest(source, component, target, arch),
            "toolchainSha256": tool_digest,
        })
    run(["npm", "ci", "--no-audit", "--no-fund"], cwd=root / "frontend", env=env)
    args = [wails_binary, "build", "-platform", f"{target}/{arch}", "-trimpath", "-m"]
    if target == "linux":
        args += ["-tags", "webkit2_41"]
    if target == "windows":
        args += ["-webview2", "error"]
    run(args, cwd=root, env=env)
    write_json(receipts / "launcher.build.json", launcher_receipt(root, manifest, target, arch, tool_record))
    write_json(receipts / "toolchain.build.json", tool_record)


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


def cargo_target(source):
    override = os.environ.get("CARGO_TARGET_DIR")
    return Path(override) if override else source / "target"


def payload_executable(source, component, target, arch=None):
    if component["repository"] == "modbus":
        folder = source / component["project"] / "build/bin"
        if target == "darwin":
            return app_entry(folder / (component["product"] + ".app"))[0]
        return folder / (component["binary"] + (".exe" if target == "windows" else ""))
    base = cargo_target(source)
    if target == "windows" and arch:
        triple = "aarch64-pc-windows-msvc" if arch == "arm64" else "x86_64-pc-windows-msvc"
        base = base / triple
    if target == "darwin":
        return app_entry(base / "release/bundle/macos" / (component["product"] + ".app"))[0]
    return base / "release" / (component["binary"] + (".exe" if target == "windows" else ""))


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
        config = json.loads((source / component["project"] / "tauri.conf.json").read_text(encoding="utf-8"))
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
    name = "SAIO.exe" if target == "windows" else "SAIO"
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


def launcher_receipt(root, manifest, target, arch, tool_record):
    config = json.loads((root / "wails.json").read_text(encoding="utf-8"))
    if config["info"]["productVersion"] != manifest["suiteVersion"]:
        raise SuiteError("Launcher source version does not match the suite manifest")
    if target == "darwin":
        app = root / "build/bin/Simulator All-in-One.app"
        entry, _ = app_entry(app)
        snapshot = tree_snapshot(app)
    else:
        with tempfile.TemporaryDirectory(prefix="simlab-launcher-") as directory:
            entry = copy_launcher(root, target, Path(directory))
            snapshot = tree_snapshot(Path(directory))
    entry = root / "build/bin/Simulator All-in-One.app/Contents/MacOS/SAIO" if target == "darwin" else root / "build/bin" / ("SAIO.exe" if target == "windows" else "SAIO")
    return {"suiteVersion": manifest["suiteVersion"], "platform": target, "architecture": arch,
            "sourceFingerprint": launcher_source_fingerprint(root), "sha256": digest(entry),
            "payloadTreeSha256": tree_digest(snapshot),
            "toolchainSha256": build_toolchain_digest(tool_record)}


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


def collect(target, arch, root=ROOT, identity="-"):
    manifest = read_manifest(root)
    assert_clean_worktree(root)
    head = source_commit(root)
    receipts = root / "dist/receipts"
    tool_stamp = receipts / "toolchain.build.json"
    if not tool_stamp.is_file():
        raise SuiteError("Missing build toolchain record; run a complete native build")
    tool_record = validate_build_toolchain(json.loads(tool_stamp.read_text(encoding="utf-8")), root, target, arch)
    stamp = receipts / "launcher.build.json"
    if not stamp.is_file() or json.loads(stamp.read_text(encoding="utf-8")) != launcher_receipt(root, manifest, target, arch, tool_record):
        raise SuiteError("Missing or stale launcher source/version/payload receipt")
    output = root / "dist/staging" / f"{target}-{arch}"
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".collect-", dir=output.parent) as directory:
        temporary = Path(directory)
        if target == "darwin":
            source = root / "build/bin/Simulator All-in-One.app"
            root_entry, info = app_entry(source)
            if info.get("CFBundleShortVersionString") != manifest["suiteVersion"]:
                raise SuiteError("Launcher version does not match components.json")
            if tuple(map(int, info.get("LSMinimumSystemVersion", "0").split("."))) < (13, 0):
                raise SuiteError("The suite launcher must declare macOS 13.0 or later")
            assert_architecture(root_entry, target, arch)
            app = temporary / "Simulator All-in-One.app"
            shutil.copytree(source, app, symlinks=True)
            suite_root = app / "Contents"
            (suite_root / "Helpers").mkdir(exist_ok=True)
        else:
            suite_root = temporary
            source = root / "build/bin" / ("SAIO.exe" if target == "windows" else "SAIO")
            if not source.is_file():
                raise SuiteError(f"Missing launcher: {source}")
            assert_architecture(source, target, arch)
            copy_launcher(root, target, suite_root)
        launcher_files = tree_snapshot(suite_root) if target != "darwin" else None
        entries = []
        for component in manifest["components"]:
            source = root / "components" / component["repository"]
            original = payload_executable(source, component, target, arch)
            if not original.is_file():
                raise SuiteError(f"Missing required role {component['id']}: {original}")
            assert_architecture(original, target, arch)
            receipt = receipts / (component["id"] + ".build.json")
            expected = {"commit": head, "platform": target, "architecture": arch, "sha256": digest(original), "suiteBuild": True, "suiteVersion": manifest["suiteVersion"], "payloadTreeSha256": source_payload_digest(source, component, target, arch), "toolchainSha256": build_toolchain_digest(tool_record)}
            if not receipt.is_file() or json.loads(receipt.read_text(encoding="utf-8")) != expected:
                raise SuiteError(f"Missing or stale suite-build receipt: {component['id']}")
            if target == "darwin":
                source_app = original.parents[2]
                _, info = app_entry(source_app)
                if info.get("CFBundleIdentifier") != component["bundleId"] or info.get("CFBundleShortVersionString") != manifest["suiteVersion"]:
                    raise SuiteError(f"Child bundle identity/version mismatch: {component['id']}")
                child = suite_root / "Helpers" / (component["product"] + ".app")
                shutil.copytree(source_app, child, symlinks=True)
                sign_app(child, identity)
                executable, _ = app_entry(child)
                payload_root = child
            else:
                executable = copy_portable(source, component, target, suite_root / "apps" / component["id"], arch)
                payload_root = executable.parent
            entries.append({"id": component["id"], "version": manifest["suiteVersion"],
                            "sourceCommit": head, "executable": executable.relative_to(suite_root).as_posix(), "sha256": digest(executable), "suiteBuild": True,
                            "payloadRoot": payload_root.relative_to(suite_root).as_posix(), "payloadFiles": tree_snapshot(payload_root)})
        assert_tree_links(suite_root)
        manifest_path = suite_root / ("Resources/suite-manifest.json" if target == "darwin" else "suite-manifest.json")
        manifest_value = {"schemaVersion": 1, "suiteVersion": manifest["suiteVersion"], "sourceCommit": head, "platform": target, "architecture": arch, "components": entries}
        if tool_record is not None:
            manifest_value["buildToolchain"] = tool_record
        if launcher_files is not None:
            manifest_value["launcherFiles"] = launcher_files
        write_json(manifest_path, manifest_value)
        if target == "darwin":
            # Child hashes are final; only sign the outer app now.
            run(["codesign", "--force", "--sign", identity, app])
            run(["codesign", "--verify", "--deep", "--strict", app])
        verify_manifest(suite_root, manifest, root=root)
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


def verify_manifest(suite_root, manifest=None, root=ROOT):
    manifest_path = suite_root / ("Resources/suite-manifest.json" if suite_root.name == "Contents" else "suite-manifest.json")
    value = json.loads(manifest_path.read_text(encoding="utf-8"))
    entries = value.get("components", [])
    if value.get("schemaVersion") != 1 or len(entries) != len(EXPECTED_IDS) or {item["id"] for item in entries} != EXPECTED_IDS:
        raise SuiteError("Manifest is incomplete or contains duplicate/unknown roles")
    expected = {item["id"]: item for item in manifest["components"]} if manifest else {}
    if value.get("platform") not in ("darwin", "windows", "linux") or value.get("architecture") not in ("amd64", "arm64"):
        raise SuiteError("Unsupported manifest target")
    if manifest and value.get("suiteVersion") != manifest["suiteVersion"]:
        raise SuiteError("Suite version mismatch")
    if "buildToolchain" in value:
        validate_build_toolchain(value["buildToolchain"], root, value["platform"], value["architecture"])
    for relative, expected_file in value.get("launcherFiles", {}).items():
        file = safe_path(suite_root, relative)
        if not file.is_file() or digest(file) != expected_file["sha256"]:
            raise SuiteError(f"Launcher resource changed: {relative}")
    for item in entries:
        executable = safe_path(suite_root, item["executable"])
        if not item.get("suiteBuild") or not re.fullmatch(r"[0-9a-f]{40}", item.get("sourceCommit", "")):
            raise SuiteError(f"Missing suite build/source identity: {item['id']}")
        if item.get("version") != value["suiteVersion"]:
            raise SuiteError(f"Component must follow the suite version: {item['id']}")
        if not executable.is_file() or digest(executable) != item["sha256"]:
            raise SuiteError(f"Payload missing or checksum mismatch: {item['id']}")
        if "payloadFiles" in item and tree_snapshot(safe_path(suite_root, item["payloadRoot"])) != item["payloadFiles"]:
            raise SuiteError(f"Payload resource tree changed: {item['id']}")
        if manifest:
            component = expected[item["id"]]
            if item["sourceCommit"] != value.get("sourceCommit"):
                raise SuiteError(f"Component source commit mismatch: {item['id']}")
            if value["platform"] == "darwin":
                _, info = app_entry(executable.parents[2])
                if info.get("CFBundleIdentifier") != component["bundleId"] or info.get("CFBundleShortVersionString") != manifest["suiteVersion"]:
                    raise SuiteError(f"Component bundle identity mismatch: {item['id']}")
    assert_tree_links(suite_root)
    return value


def suite_root_for(root, target, arch):
    stage = root / "dist/staging" / f"{target}-{arch}"
    return stage / "Simulator All-in-One.app/Contents" if target == "darwin" else stage


def package(target, arch, root=ROOT, webview2=None, webview2_sha256=None):
    manifest = read_manifest(root)
    stage = root / "dist/staging" / f"{target}-{arch}"
    suite_root = suite_root_for(root, target, arch)
    manifest_value = verify_manifest(suite_root, manifest, root=root)
    if manifest_value["platform"] != target or manifest_value["architecture"] != arch:
        raise SuiteError("Package target does not match the staged manifest")
    output = root / "dist/packages"
    output.mkdir(parents=True, exist_ok=True)
    name = f"SAIO_{manifest['suiteVersion']}_{target}-{arch}"
    if target == "darwin":
        require("ditto")
        require("hdiutil")
        run(["codesign", "--verify", "--deep", "--strict", stage / "Simulator All-in-One.app"])
        zip_path = output / (name + ".zip")
        run(["ditto", "-c", "-k", "--sequesterRsrc", "--keepParent", stage / "Simulator All-in-One.app", zip_path])
        write_json(zip_path.with_suffix(".zip.sha256.json"), {"file": zip_path.name, "sha256": digest(zip_path)})
        destination = output / (name + ".dmg")
        with tempfile.TemporaryDirectory(prefix=".dmg-", dir=output) as directory:
            image_root = Path(directory)
            shutil.copytree(stage / "Simulator All-in-One.app", image_root / "Simulator All-in-One.app", symlinks=True)
            (image_root / "Applications").symlink_to("/Applications")
            run(["hdiutil", "create", "-ov", "-format", "UDZO", "-volname", "SAIO", "-srcfolder", image_root, destination])
        run(["hdiutil", "verify", destination])
    elif target == "windows":
        if arch != "amd64":
            raise SuiteError("The single-EXE package currently supports Windows x64")
        if webview2 is not None or webview2_sha256 is not None:
            raise SuiteError("Portable Windows packages do not bundle WebView2; publish its installer separately")
        try:
            toolchain.check_tools(toolchain.load_lock(root / "toolchain.lock.json"), names=("go",), platform=target)
        except toolchain.ToolchainError as error:
            raise SuiteError(str(error)) from error
        destination = output / (name + "-portable.exe")
        windows_portable.build(stage, destination, manifest["suiteVersion"], root, run, digest)
        assert_architecture(destination, target, arch)
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
        index.write_text("".join(indexed), encoding="utf-8")
        with tempfile.NamedTemporaryFile(suffix=".tar.gz") as temporary:
            with tarfile.open(temporary.name, "w:gz") as archive:
                archive.add(stage, arcname="SAIO", recursive=True)
            payload_hash = digest(temporary.name)
            header = (root / "packaging/linux/self-extract.sh").read_text(encoding="utf-8").replace("@PAYLOAD_SHA256@", payload_hash).replace("@INDEX_SHA256@", digest(index)).replace("@VERSION@", manifest["suiteVersion"]).replace("@ARCH@", arch)
            with destination.open("wb") as stream:
                stream.write(header.encode())
                stream.write(Path(temporary.name).read_bytes())
        destination.chmod(0o755)
    write_json(destination.with_suffix(destination.suffix + ".sha256.json"), {"file": destination.name, "sha256": digest(destination)})
    print(f"Package: {destination}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["icons", "build", "collect", "verify", "package"])
    parser.add_argument("--platform", choices=["darwin", "windows", "linux"], default={"Darwin": "darwin", "Windows": "windows", "Linux": "linux"}.get(host_platform.system()))
    parser.add_argument("--arch", choices=["arm64", "amd64"], default="arm64" if host_platform.machine().lower() in ("arm64", "aarch64") else "amd64")
    parser.add_argument("--signing-identity", default=os.environ.get("SIMLAB_SIGNING_IDENTITY", "-"))
    parser.add_argument("--webview2", type=Path)
    parser.add_argument("--webview2-sha256")
    args = parser.parse_args()
    if args.command == "icons":
        inject_icons()
        print("Injected role icons into all component trees")
    elif args.command == "build":
        build(args.platform, args.arch)
    elif args.command == "collect":
        collect(args.platform, args.arch, identity=args.signing_identity)
    elif args.command == "verify":
        value = verify_manifest(suite_root_for(ROOT, args.platform, args.arch), read_manifest())
        print(f"Verified {len(value['components'])}/{len(EXPECTED_IDS)} roles for {args.platform}/{args.arch}")
    else:
        package(args.platform, args.arch, webview2=args.webview2, webview2_sha256=args.webview2_sha256)


if __name__ == "__main__":
    try:
        main()
    except (SuiteError, OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        sys.exit(1)
