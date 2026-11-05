#!/usr/bin/env python3
"""Inspect the pinned build toolchain. Python 3.10+; standard library only.

Lock digest convention: SHA-256 over the UTF-8 encoding of
``json.dumps(validated_lock, sort_keys=True, separators=(",", ":"), ensure_ascii=False)``,
where ``validated_lock`` is the normalized structure returned by ``load_lock``
(known fields only, tool names sorted, platform lists sorted). Formatting changes
in the file therefore do not change the digest, while any locked version,
platform or schema change does.

The helper never installs, upgrades or replaces tools. Commands are fixed in
``TOOLS``; nothing in the lock file is executed. Process environments are not
collected, and version commands run without a shell under a short timeout.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
SCHEMA_VERSION = 1
COMMAND_TIMEOUT = 15
SUPPORTED_PLATFORMS = ("darwin", "linux", "windows")

_VERSION_PATTERN = re.compile(r"^\d+(?:\.\d+)+(?:[-+][0-9A-Za-z][0-9A-Za-z.]*)?$")
_VERSION_TOKEN = r"(\d+\.\d+(?:\.\d+)?(?:[-+][0-9A-Za-z][0-9A-Za-z.]*)?)"

# Fixed command mappings. ``python`` always uses the interpreter running this
# helper, so an unrelated interpreter earlier on PATH cannot be reported.
TOOLS = {
    "node": {"command": "node", "args": ("--version",), "patterns": (_VERSION_TOKEN,)},
    "npm": {"command": "npm", "args": ("--version",), "patterns": (_VERSION_TOKEN,)},
    "python": {
        "command": None,
        "args": ("--version",),
        "patterns": (r"Python\s+" + _VERSION_TOKEN, _VERSION_TOKEN),
    },
    "go": {
        "command": "go",
        "args": ("version",),
        "patterns": (r"go version go\s*" + _VERSION_TOKEN, _VERSION_TOKEN),
    },
    "rust": {
        "command": "rustc",
        "args": ("--version",),
        "patterns": (r"rustc\s+" + _VERSION_TOKEN, _VERSION_TOKEN),
    },
    "wails": {"command": "wails", "args": ("version",), "patterns": (_VERSION_TOKEN,)},
    "nsis": {"command": "makensis", "args": ("-VERSION",), "patterns": (_VERSION_TOKEN,)},
}
TOOL_ORDER = tuple(TOOLS)


class ToolchainError(RuntimeError):
    """A lock, platform or tool inspection problem with an actionable message."""


def normalize_platform(platform: str | None = None) -> str:
    """Return ``darwin``, ``linux`` or ``windows`` for a sys.platform value."""
    raw = sys.platform if platform is None else platform
    if not isinstance(raw, str):
        raise ToolchainError(f"Unsupported platform: {raw!r}")
    value = raw.strip().lower()
    if value == "darwin":
        return "darwin"
    if value.startswith("linux"):
        return "linux"
    if value in ("win32", "windows", "cygwin", "msys"):
        return "windows"
    raise ToolchainError(f"Unsupported platform: {raw!r}; expected darwin, linux or windows")


def _reject_control(value: str, label: str) -> None:
    if any(ord(character) < 0x20 or ord(character) == 0x7F for character in value):
        raise ToolchainError(f"Refusing {label} containing control characters")


def validate_lock(lock: object) -> dict:
    """Validate and normalize a lock document, raising ToolchainError on bad input."""
    if not isinstance(lock, dict):
        raise ToolchainError("Toolchain lock must be a JSON object")
    if "schemaVersion" not in lock:
        raise ToolchainError("Toolchain lock is missing schemaVersion")
    version = lock["schemaVersion"]
    if isinstance(version, bool) or not isinstance(version, int):
        raise ToolchainError(f"Invalid toolchain schemaVersion: {version!r}")
    if version != SCHEMA_VERSION:
        raise ToolchainError(
            f"Unsupported toolchain schema version {version}; this helper supports {SCHEMA_VERSION}"
        )
    raw_tools = lock.get("tools")
    if not isinstance(raw_tools, dict):
        raise ToolchainError("Toolchain lock must contain a tools object")
    if not raw_tools:
        raise ToolchainError("Toolchain lock contains no tools")

    normalized_tools: dict[str, dict] = {}
    for name in sorted(raw_tools):
        if not isinstance(name, str) or name not in TOOLS:
            raise ToolchainError(
                f"Unknown tool in toolchain lock: {name!r}; known tools: {', '.join(TOOL_ORDER)}"
            )
        spec = raw_tools[name]
        if not isinstance(spec, dict):
            raise ToolchainError(f"Tool {name} must be a JSON object")
        version = spec.get("version")
        if not isinstance(version, str) or not version:
            raise ToolchainError(f"Tool {name} is missing a version string")
        _reject_control(version, f"version for {name}")
        if not _VERSION_PATTERN.match(version):
            raise ToolchainError(
                f"Tool {name} has an invalid version {version!r}; use a dotted numeric version"
            )
        normalized = {"version": version}
        if "platforms" in spec:
            platforms = spec["platforms"]
            if not isinstance(platforms, list) or not platforms:
                raise ToolchainError(f"Tool {name} platforms must be a non-empty list")
            for platform in platforms:
                if not isinstance(platform, str) or platform not in SUPPORTED_PLATFORMS:
                    raise ToolchainError(
                        f"Tool {name} names an unsupported platform {platform!r}; "
                        f"expected one of {', '.join(SUPPORTED_PLATFORMS)}"
                    )
            normalized["platforms"] = sorted(set(platforms))
        normalized_tools[name] = normalized
    return {"schemaVersion": SCHEMA_VERSION, "tools": normalized_tools}


def load_lock(path: str | Path) -> dict:
    """Read, validate and normalize a toolchain lock file."""
    lock_path = Path(path)
    try:
        text = lock_path.read_text(encoding="utf-8")
    except OSError as error:
        raise ToolchainError(f"Cannot read toolchain lock {lock_path}: {error}") from error
    try:
        raw = json.loads(text)
    except json.JSONDecodeError as error:
        raise ToolchainError(f"Invalid JSON in toolchain lock {lock_path}: {error}") from error
    try:
        return validate_lock(raw)
    except ToolchainError as error:
        raise ToolchainError(f"{lock_path}: {error}") from error


def _canonical(lock: object) -> bytes:
    validated = validate_lock(lock)
    return json.dumps(
        validated, sort_keys=True, separators=(",", ":"), ensure_ascii=False
    ).encode("utf-8")


def _digest_value(lock: object) -> str:
    return hashlib.sha256(_canonical(lock)).hexdigest()


def lock_digest(path: str | Path) -> str:
    """Return the canonical SHA-256 digest of the validated lock file."""
    return _digest_value(load_lock(path))


def _applicable(spec: dict, platform_name: str) -> bool:
    return "platforms" not in spec or platform_name in spec["platforms"]


def _parse_names(names: object) -> list[str]:
    if isinstance(names, str):
        candidates = names.split(",")
    else:
        try:
            candidates = list(names)  # type: ignore[arg-type]
        except TypeError as error:
            raise ToolchainError(f"Invalid tool selection: {names!r}") from error
    selected: list[str] = []
    for candidate in candidates:
        if not isinstance(candidate, str):
            raise ToolchainError(f"Invalid tool name: {candidate!r}")
        name = candidate.strip()
        if not name:
            continue
        _reject_control(name, "tool name")
        if name not in TOOLS:
            raise ToolchainError(
                f"Unknown tool {name!r}; known tools: {', '.join(TOOL_ORDER)}"
            )
        if name not in selected:
            selected.append(name)
    if not selected:
        raise ToolchainError("No tools selected")
    return selected


def _select_tools(lock: dict, names: object, platform_name: str) -> list[str]:
    locked = lock["tools"]
    if names is None:
        selected = [
            name
            for name in TOOL_ORDER
            if name in locked and _applicable(locked[name], platform_name)
        ]
        if not selected:
            raise ToolchainError(f"No locked tools apply to {platform_name}")
        return selected
    selected = _parse_names(names)
    for name in selected:
        if name not in locked:
            raise ToolchainError(f"Tool {name} is not defined in the toolchain lock")
        if not _applicable(locked[name], platform_name):
            supported = ", ".join(locked[name]["platforms"])
            raise ToolchainError(
                f"Tool {name} is only supported on {supported}, not {platform_name}"
            )
    return selected


def _absolute(path: object) -> str:
    """Make a path absolute without resolving symlinks or changing argv[0]."""
    return os.path.abspath(os.fspath(path))


def _which(command: str) -> str:
    found = shutil.which(command)
    if not found:
        raise ToolchainError(f"{command} is not installed or not on PATH")
    return _absolute(found)


def _wails_from_gopath(platform_name: str) -> str:
    """Resolve the suite.py GOPATH fallback under a bounded, shell-free call."""
    try:
        go = _which("go")
    except ToolchainError as error:
        raise ToolchainError(
            f"Wails CLI is not on PATH and its GOPATH fallback needs go: {error}"
        ) from error
    try:
        output = _run([go, "env", "GOPATH"], include_stderr=False)
    except ToolchainError as error:
        raise ToolchainError(f"Cannot resolve Wails through GOPATH: {error}") from error
    roots = [root.strip() for root in output.strip().split(os.pathsep) if root.strip()]
    if not roots:
        raise ToolchainError(
            "Cannot resolve Wails: go env GOPATH returned no path; "
            "install the locked CLI with go install github.com/wailsapp/wails/v2/cmd/wails"
        )
    binary = Path(roots[0]) / "bin" / ("wails.exe" if platform_name == "windows" else "wails")
    if not binary.is_file():
        raise ToolchainError(
            f"Wails CLI is not installed at {binary}; install the locked CLI with "
            "go install github.com/wailsapp/wails/v2/cmd/wails"
        )
    return _absolute(binary)


def _wails_executable(platform_name: str) -> str:
    found = shutil.which("wails")
    if found:
        return _absolute(found)
    return _wails_from_gopath(platform_name)


def _invocation(tool: str, platform_name: str) -> tuple[str, list[str]]:
    spec = TOOLS[tool]
    if tool == "python":
        if not sys.executable:
            raise ToolchainError("Cannot determine the running Python interpreter")
        path = _absolute(sys.executable)
        return path, [path, "--version"]
    if tool == "wails":
        path = _wails_executable(platform_name)
        return path, [path, *spec["args"]]
    command = spec["command"]
    path = _which(command)
    if tool == "npm" and platform_name == "windows":
        # npm.cmd is a batch shim; invoke its JavaScript entry point with node,
        # matching scripts/suite.py, so no shell is involved.
        node = _which("node")
        cli = Path(path).parent / "node_modules" / "npm" / "bin" / "npm-cli.js"
        if not cli.is_file():
            raise ToolchainError(f"Cannot resolve the npm Node entry point: {cli}")
        return path, [node, str(cli), "--version"]
    return path, [path, *spec["args"]]


def _text(value: object) -> str:
    if value is None:
        return ""
    if isinstance(value, bytes):
        return value.decode("utf-8", "replace")
    return str(value)


def _snippet(text: str, limit: int = 300) -> str:
    compact = " ".join(text.split())
    if len(compact) > limit:
        compact = compact[:limit] + "..."
    return compact


def _run(argv: list[str], include_stderr: bool = True) -> str:
    try:
        completed = subprocess.run(
            argv,
            shell=False,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=COMMAND_TIMEOUT,
            check=False,
        )
    except subprocess.TimeoutExpired as error:
        raise ToolchainError(
            f"{Path(argv[0]).name} timed out after {COMMAND_TIMEOUT}s"
        ) from error
    except FileNotFoundError as error:
        raise ToolchainError(f"Executable disappeared: {argv[0]}") from error
    except OSError as error:
        raise ToolchainError(f"Cannot run {argv[0]}: {error}") from error
    output = _text(completed.stdout)
    if include_stderr:
        output += "\n" + _text(completed.stderr)
    if completed.returncode != 0:
        diagnostics = _text(completed.stdout) + "\n" + _text(completed.stderr)
        raise ToolchainError(
            f"{Path(argv[0]).name} exited with status {completed.returncode}: "
            f"{_snippet(diagnostics) or '<no output>'}"
        )
    return output


def _run_variants(tool: str, base_argv: list[str], platform_name: str) -> str:
    # ``None`` keeps the mapped arguments; a string replaces the mapped switch.
    variants: list[str | None] = [None]
    if tool == "nsis" and platform_name == "windows":
        # Some makensis builds accept only the Windows-style switch.
        variants = ["-VERSION", "/VERSION"]
    errors: list[ToolchainError] = []
    for extra in variants:
        argv = base_argv if extra is None else base_argv[:-1] + [extra]
        try:
            return _run(argv)
        except ToolchainError as error:
            errors.append(error)
    raise ToolchainError("; ".join(str(error) for error in errors))


def _parse_version(tool: str, output: str) -> str:
    for pattern in TOOLS[tool]["patterns"]:
        match = re.search(pattern, output, re.MULTILINE)
        if match:
            return match.group(1)
    raise ToolchainError(
        f"Cannot parse {tool} version from output: {_snippet(output) or '<no output>'}"
    )


def _version_value(version: str) -> str:
    return version[1:] if version[:1] in ("v", "V") else version


def _normalize_nsis_version(version: str) -> str:
    """Drop trailing zero release components; keep any prerelease/build suffix."""
    value = _version_value(version)
    match = re.match(r"^(\d+(?:\.\d+)*)(.*)$", value)
    if not match:
        return value
    release, suffix = match.groups()
    components = release.split(".")
    while len(components) > 1 and components[-1] == "0":
        components.pop()
    return ".".join(components) + suffix


def _same_version(tool: str, actual: str, expected: str) -> bool:
    if tool == "nsis":
        # NSIS reports 3.13 while Chocolatey/lock pins 3.13.0.
        return _normalize_nsis_version(actual) == _normalize_nsis_version(expected)
    return _version_value(actual) == _version_value(expected)


def inspect_tools(lock: dict, names: object = None, platform: str | None = None) -> list[dict]:
    """Inspect locked tools and return JSON-serializable records.

    Tools that the lock explicitly limits to other platforms are skipped when
    ``names`` is omitted. Selecting such a tool explicitly raises ToolchainError
    instead of reporting success. Inspection failures are recorded in the
    per-tool ``matches``/``error`` fields so callers can report every problem at
    once; ``check_tools`` turns them into a single actionable error.
    """
    validated = validate_lock(lock)
    platform_name = normalize_platform(platform)
    records: list[dict] = []
    for name in _select_tools(validated, names, platform_name):
        expected = validated["tools"][name]["version"]
        record = {
            "tool": name,
            "expected": expected,
            "actual": None,
            "path": None,
            "matches": False,
        }
        try:
            path, base_argv = _invocation(name, platform_name)
            record["path"] = path
            actual = _parse_version(
                name, _run_variants(name, base_argv, platform_name)
            )
            record["actual"] = actual
            record["matches"] = _same_version(name, actual, expected)
            if not record["matches"]:
                record["error"] = "version mismatch"
        except ToolchainError as error:
            record["error"] = str(error)
        records.append(record)
    return records


def check_tools(lock: dict, names: object = None, platform: str | None = None) -> list[dict]:
    """Return inspection records, raising on the first batch of mismatches."""
    records = inspect_tools(lock, names=names, platform=platform)
    problems = [record for record in records if not record["matches"]]
    if not problems:
        return records
    details = []
    for record in problems:
        found = record["actual"] or "not available"
        location = record["path"] or "not found"
        detail = (
            f"{record['tool']}: expected {record['expected']}, "
            f"found {found} at {location}"
        )
        if record.get("error"):
            detail += f" ({record['error']})"
        details.append(detail)
    raise ToolchainError(
        "Toolchain does not match the lock; install the pinned versions or update "
        "toolchain.lock.json:\n  " + "\n  ".join(details)
    )


def _check_output_path(value: str) -> Path:
    if not isinstance(value, str) or not value:
        raise ToolchainError("An output path is required")
    _reject_control(value, "output path")
    return Path(value)


def _write_json(path: Path, document: dict) -> None:
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(
            json.dumps(document, indent=2, ensure_ascii=False) + "\n",
            encoding="utf-8",
        )
    except OSError as error:
        raise ToolchainError(f"Cannot write {path}: {error}") from error


def _append_lines(path: Path, lines: list[str]) -> None:
    for line in lines:
        _reject_control(line, "output line")
        if "=" not in line or "\n" in line:
            raise ToolchainError("Refusing malformed CI output line")
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        with path.open("a", encoding="utf-8", newline="\n") as stream:
            for line in lines:
                stream.write(line + "\n")
    except OSError as error:
        raise ToolchainError(f"Cannot write {path}: {error}") from error


def _add_common_arguments(parser: argparse.ArgumentParser, suppress: bool = False) -> None:
    default = argparse.SUPPRESS if suppress else str(ROOT / "toolchain.lock.json")
    parser.add_argument(
        "--lock",
        default=default,
        metavar="PATH",
        help="toolchain lock file (default: repository toolchain.lock.json)",
    )
    parser.add_argument(
        "--tools",
        default=argparse.SUPPRESS if suppress else None,
        metavar="LIST",
        help="comma-separated tool subset (default: every applicable locked tool)",
    )


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="toolchain.py",
        description="Inspect and record the pinned build toolchain.",
    )
    _add_common_arguments(parser)
    subparsers = parser.add_subparsers(dest="command", required=True)
    check = subparsers.add_parser("check", help="verify installed tools against the lock")
    _add_common_arguments(check, suppress=True)
    record = subparsers.add_parser(
        "record", help="verify tools, then write actual versions as JSON"
    )
    _add_common_arguments(record, suppress=True)
    record.add_argument("--output", required=True, metavar="PATH", help="JSON output file")
    ci = subparsers.add_parser(
        "ci", help="emit locked versions as name=value lines without inspecting tools"
    )
    _add_common_arguments(ci, suppress=True)
    ci.add_argument("--output", required=True, metavar="PATH", help="GitHub output file")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        lock = load_lock(args.lock)
        if args.command == "ci":
            platform_name = normalize_platform()
            selected = _select_tools(lock, args.tools, platform_name)
            lines = [
                f"{name}={lock['tools'][name]['version']}" for name in selected
            ]
            output = _check_output_path(args.output)
            _append_lines(output, lines)
            print(f"toolchain: wrote {len(lines)} locked version(s) to {output}")
            return 0
        records = check_tools(lock, names=args.tools)
        if args.command == "record":
            output = _check_output_path(args.output)
            document = {
                "schemaVersion": SCHEMA_VERSION,
                "lockSha256": _digest_value(lock),
                "platform": normalize_platform(),
                "tools": {record["tool"]: record["actual"] for record in records},
            }
            _write_json(output, document)
            print(f"toolchain: wrote {len(records)} recorded version(s) to {output}")
            return 0
        for record in records:
            print(f"{record['tool']}: {record['actual']} ({record['path']})")
        print(f"toolchain: {len(records)} selected tool(s) match the lock")
        return 0
    except ToolchainError as error:
        print(f"toolchain: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
