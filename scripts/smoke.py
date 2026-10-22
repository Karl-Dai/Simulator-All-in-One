#!/usr/bin/env python3
"""Startup smoke checks; process survival is not protocol/UI acceptance."""
import argparse
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time

LOG_TAIL_BYTES = 64 * 1024


def read_log_tail(path):
    """Read one bounded snapshot without changing the process writer's offset."""
    with Path(path).open("rb") as reader:
        size = os.fstat(reader.fileno()).st_size
        count = min(size, LOG_TAIL_BYTES)
        reader.seek(size - count)
        return reader.read(count).decode("utf-8", errors="replace")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("stage", type=Path)
    parser.add_argument("--seconds", type=int, default=8)
    args = parser.parse_args()
    root = args.stage.resolve()
    manifest = json.loads((root / "suite-manifest.json").read_text(encoding="utf-8"))
    with tempfile.TemporaryDirectory(prefix="simlab-smoke-") as directory:
        env = os.environ.copy()
        env["SIMLAB_DATA_DIR"] = str(Path(directory) / "data")
        linux = manifest["platform"] == "linux"
        items = [("launcher", root / ("SimLab.exe" if os.name == "nt" else "SimLab"))]
        items += [(item["id"], root / item["executable"]) for item in manifest["components"]]
        for role, executable in items:
            command = [str(executable)]
            if linux:
                command = [str(root / "run-simlab.sh")]
                if role != "launcher":
                    command += ["--component", role]
            log = Path(directory) / (role + ".log")
            with log.open("wb") as writer:
                print(f"START {role}: launching {executable.name}", flush=True)
                process = subprocess.Popen(command, cwd=directory, env=env, stdout=writer, stderr=subprocess.STDOUT,
                                           start_new_session=os.name != "nt")
                try:
                    time.sleep(args.seconds)
                    logs = read_log_tail(log)
                    if process.poll() is not None:
                        raise RuntimeError(f"{role} exited during startup ({process.returncode}):\n{logs}")
                    if any(term in logs for term in ("error while loading shared libraries", "Unable to spawn a new child process", "Could not create default EGL display")):
                        raise RuntimeError(f"{role} failed to initialize WebKit:\n{logs}")
                    print(f"PASS {role}: process survived {args.seconds}s", flush=True)
                finally:
                    if os.name == "nt":
                        subprocess.run(["taskkill", "/F", "/T", "/PID", str(process.pid)], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
                    else:
                        try:
                            os.killpg(process.pid, signal.SIGTERM)
                        except ProcessLookupError:
                            pass
                    try:
                        process.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        process.kill()


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, RuntimeError) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        sys.exit(1)
