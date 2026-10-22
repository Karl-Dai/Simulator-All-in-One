#!/usr/bin/env python3
"""Collect and audit Ubuntu 22.04 GTK3/WebKitGTK 4.1 runtime closure."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys

HOST_LIBS = re.compile(r"^(?:ld-linux.*|lib(?:c|m|dl|pthread|rt|resolv|util|anl|nss_[^.]+)\.so.*|lib(?:EGL|GL|GLX|GLES.*|OpenGL|gbm|drm.*|wayland-.*|vulkan)\.so.*)$")


def sha(path):
    value = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            value.update(block)
    return value.hexdigest()


def command(args, env=None):
    return subprocess.check_output(list(map(str, args)), env=env, text=True, stderr=subprocess.STDOUT)


def elf(path):
    if not path.is_file():
        return False
    with path.open("rb") as stream:
        return stream.read(4) == b"\x7fELF"


def dependencies(path, environment):
    try:
        result = command(["ldd", path], env=environment)
    except subprocess.CalledProcessError as error:
        if "not a dynamic executable" in error.output or "statically linked" in error.output:
            return []
        raise
    if "not found" in result:
        raise RuntimeError(f"Unresolved runtime dependency for {path}:\n{result}")
    output = []
    for line in result.splitlines():
        match = re.search(r"=>\s+(/\S+)", line) or re.match(r"\s*(/\S+)", line)
        if match:
            output.append(Path(match.group(1)))
    return output


def environment(stage):
    value = os.environ.copy()
    value["LD_LIBRARY_PATH"] = str(stage / "lib")
    return value


def runtime_files(stage):
    return [path for parent in (stage / "lib", stage / "runtime") for path in parent.rglob("*") if path.is_file() and path.name != "closure.json"]


def audit(stage, require_record=True):
    record_path = stage / "runtime/closure.json"
    if require_record:
        if not record_path.is_file():
            raise RuntimeError("Runtime closure has not been prepared; an ELF-only payload is incomplete")
        record = json.loads(record_path.read_text())
        if record.get("baseline") != "ubuntu-22.04-amd64" or not record.get("complete"):
            raise RuntimeError("Unsupported or incomplete Linux runtime closure")
        actual = {path.relative_to(stage).as_posix() for path in runtime_files(stage)} | {"run-simlab.sh"}
        if actual != set(record["files"]):
            raise RuntimeError("Runtime closure file set changed")
        for relative, expected in record["files"].items():
            path = stage / relative
            if not path.is_file() or sha(path) != expected:
                raise RuntimeError(f"Runtime file changed or disappeared: {relative}")
        if record["launcherSha256"] != sha(stage / "SimLab"):
            raise RuntimeError("The launcher changed after runtime closure collection")
    required = ["WebKitWebProcess", "WebKitNetworkProcess", "injected-bundle/libwebkit2gtkinjectedbundle.so"]
    helper = stage / "runtime/usr/lib/x86_64-linux-gnu/webkit2gtk-4.1"
    for name in required:
        if not (helper / name).is_file():
            raise RuntimeError(f"Missing WebKitGTK runtime helper: {helper / name}")
    webkits = list((stage / "lib").glob("libwebkit2gtk-4.1.so*"))
    if not webkits or any(b"/usr/lib/x86_64-linux-gnu/webkit2gtk-4.1" in path.read_bytes() for path in webkits):
        raise RuntimeError("WebKitGTK helper paths were not relocated")
    inputs = [stage / "SimLab"] + list((stage / "apps").rglob("*")) + runtime_files(stage)
    env = environment(stage)
    host_dependencies = set()
    for path in inputs:
        if not elf(path):
            continue
        versions = re.findall(r"GLIBC_(\d+)\.(\d+)", command(["readelf", "--version-info", path]))
        if any(tuple(map(int, version)) > (2, 35) for version in versions):
            raise RuntimeError(f"Runtime requires glibc newer than the Ubuntu 22.04 baseline: {path}")
        for dependency in dependencies(path, env):
            if dependency.resolve().is_relative_to(stage.resolve()):
                continue
            if not HOST_LIBS.fullmatch(dependency.name):
                raise RuntimeError(f"Runtime closure leaks a host dependency: {path}: {dependency}")
            host_dependencies.add(dependency.name)
    return sorted(host_dependencies)


def prepare(stage):
    release = Path("/etc/os-release").read_text()
    if not re.search(r'^ID=ubuntu$', release, re.M) or not re.search(r'^VERSION_ID="22\.04"$', release, re.M):
        raise RuntimeError("Runtime collection requires native Ubuntu 22.04 amd64; do not silently raise the glibc baseline")
    if os.uname().machine != "x86_64":
        raise RuntimeError("Only the Ubuntu 22.04 amd64 closure is currently supported")
    for tool in ("ldd", "readelf", "dpkg-query"):
        if not shutil.which(tool):
            raise RuntimeError(f"Missing runtime packaging tool: {tool}")
    if (stage / "runtime").exists() or (stage / "lib").exists():
        raise RuntimeError("Start from a fresh collect stage; existing runtime data is never overwritten")
    libroot = Path("/usr/lib/x86_64-linux-gnu")
    runtime = stage / "runtime"
    libdir = stage / "lib"
    libdir.mkdir()
    required_dirs = [libroot / "webkit2gtk-4.1", libroot / "gtk-3.0", libroot / "gdk-pixbuf-2.0", libroot / "gio/modules", Path("/usr/share/glib-2.0/schemas"), Path("/usr/share/fonts/truetype/dejavu")]
    optional_dirs = [libroot / "girepository-1.0", libroot / "gstreamer-1.0", Path("/usr/share/mime"), Path("/usr/share/icons/Adwaita"), Path("/usr/share/icons/hicolor")]
    optional_dirs += list(Path("/usr/share").glob("webkit*"))
    for source in required_dirs + optional_dirs:
        if not source.is_dir():
            if source in required_dirs:
                raise RuntimeError(f"Required runtime resource directory is missing: {source}")
            continue
        target = runtime / source.relative_to("/")
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copytree(source, target, symlinks=False)
    fontconfig = runtime / "etc/fonts"
    fontconfig.mkdir(parents=True)
    (fontconfig / "fonts.conf").write_text('<?xml version="1.0"?><!DOCTYPE fontconfig SYSTEM "fonts.dtd"><fontconfig><dir prefix="relative">../../usr/share/fonts</dir><cachedir prefix="xdg">fontconfig</cachedir></fontconfig>\n')
    query_candidates = list(libroot.glob("gdk-pixbuf-2.0/*/gdk-pixbuf-query-loaders"))
    query_candidates += list(libroot.glob("gdk-pixbuf-2.0/gdk-pixbuf-query-loaders"))
    if not query_candidates:
        raise RuntimeError("gdk-pixbuf-query-loaders is required to rebuild relocatable image-loader caches")
    bindir = runtime / "bin"
    bindir.mkdir()
    shutil.copy2(query_candidates[0], bindir / "gdk-pixbuf-query-loaders")
    for source in (Path("/usr/bin/xdg-open"), Path("/usr/bin/gsettings")):
        if source.is_file():
            shutil.copy2(source, bindir / source.name)
    webkit_source = next(libroot.glob("libwebkit2gtk-4.1.so.0*"), None)
    if webkit_source is None:
        raise RuntimeError("WebKitGTK 4.1 shared library is missing")
    # ldd cannot discover executable paths used by WebKit's sandbox launcher.
    for name in ("bwrap", "xdg-dbus-proxy"):
        absolute = "/usr/bin/" + name
        if absolute.encode() in webkit_source.read_bytes():
            source = Path(absolute)
            if not source.is_file():
                raise RuntimeError(f"WebKitGTK declares a missing runtime tool: {source}")
            target = runtime / "usr/bin" / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, target)
    scanner = libroot / "gstreamer1.0/gstreamer-1.0/gst-plugin-scanner"
    if (runtime / "usr/lib/x86_64-linux-gnu/gstreamer-1.0").exists():
        if not scanner.is_file():
            raise RuntimeError(f"GStreamer plugin scanner is missing: {scanner}")
        shutil.copy2(scanner, bindir / "gst-plugin-scanner")
    sources = [path for path in stage.rglob("*") if elf(path)]
    env = environment(stage)
    visited = set()
    source_libraries = set()
    while sources:
        path = sources.pop()
        if path.resolve() in visited:
            continue
        visited.add(path.resolve())
        for dependency in dependencies(path, env):
            if HOST_LIBS.fullmatch(dependency.name):
                continue
            destination = libdir / dependency.name
            if destination.exists():
                if dependency.resolve() != destination.resolve() and sha(dependency) != sha(destination):
                    raise RuntimeError(f"Two different libraries share one SONAME: {dependency.name}")
                continue
            shutil.copy2(dependency, destination, follow_symlinks=True)
            source_libraries.add(dependency)
            sources.append(destination)
    # Match Tauri's GTK AppImage bundler's equal-length /usr -> ././ rewrite.
    # All processes must have cwd=runtime/usr; no environment override of
    # WEBKIT_EXEC_PATH is assumed in distribution builds of WebKitGTK.
    for library in libdir.glob("libwebkit*"):
        data = library.read_bytes()
        if b"/usr" not in data:
            raise RuntimeError(f"Unexpected WebKit library layout: {library}")
        library.write_bytes(data.replace(b"/usr", b"././"))
    copyright_dir = runtime / "licenses"
    copyright_dir.mkdir()
    for source in source_libraries:
        result = subprocess.run(["dpkg-query", "-S", str(source)], text=True, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
        if result.returncode == 0:
            for line in result.stdout.splitlines():
                package = line.split(": ", 1)[0].split(":", 1)[0]
                copyright_file = Path("/usr/share/doc") / package / "copyright"
                if copyright_file.is_file():
                    shutil.copy2(copyright_file, copyright_dir / (package + ".copyright"))
    wrapper = Path(__file__).with_name("run-simlab.sh")
    shutil.copy2(wrapper, stage / "run-simlab.sh")
    (stage / "run-simlab.sh").chmod(0o755)
    host_dependencies = audit(stage, require_record=False)
    files = {path.relative_to(stage).as_posix(): sha(path) for path in runtime_files(stage)}
    files["run-simlab.sh"] = sha(stage / "run-simlab.sh")
    record = {"schemaVersion": 1, "baseline": "ubuntu-22.04-amd64", "complete": True,
              "hostDependencies": host_dependencies, "launcherSha256": sha(stage / "SimLab"), "files": files}
    (runtime / "closure.json").write_text(json.dumps(record, indent=2) + "\n")
    audit(stage)
    print(f"Runtime closure audited; {len(files)} files; host boundary: {host_dependencies}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["prepare", "audit"])
    parser.add_argument("stage", type=Path)
    args = parser.parse_args()
    if args.command == "prepare":
        prepare(args.stage.resolve())
    else:
        audit(args.stage.resolve())
        print("Linux runtime closure is complete and unchanged")


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, RuntimeError, subprocess.CalledProcessError) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        sys.exit(1)
