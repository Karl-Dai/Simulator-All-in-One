#!/usr/bin/env python3
"""Fetch an explicitly pinned HTTPS build prerequisite; never use it at runtime."""
import argparse
import hashlib
from pathlib import Path
import re
import sys
import tempfile
import urllib.request

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("url")
parser.add_argument("sha256")
parser.add_argument("output", type=Path)
args = parser.parse_args()
try:
    if not args.url.startswith("https://") or not re.fullmatch(r"[0-9a-f]{64}", args.sha256):
        raise ValueError("An HTTPS URL and lowercase SHA256 are required")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(dir=args.output.parent, delete=False) as stream:
        temporary = Path(stream.name)
        checksum = hashlib.sha256()
        with urllib.request.urlopen(args.url, timeout=120) as response:
            if not response.url.startswith("https://"):
                raise ValueError("Prerequisite redirected away from HTTPS")
            for block in iter(lambda: response.read(1024 * 1024), b""):
                stream.write(block)
                checksum.update(block)
    if checksum.hexdigest() != args.sha256:
        raise ValueError("Downloaded prerequisite checksum mismatch")
    temporary.replace(args.output)
except Exception as error:
    if "temporary" in locals():
        temporary.unlink(missing_ok=True)
    print(f"ERROR: {error}", file=sys.stderr)
    sys.exit(1)
