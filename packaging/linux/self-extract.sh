#!/bin/sh
set -eu
expected_sha='@PAYLOAD_SHA256@'
expected_index='@INDEX_SHA256@'
version='@VERSION@'
architecture='@ARCH@'
archive_line="$(awk '/^__SIMLAB_ARCHIVE_BELOW__$/ {print NR + 1; exit}' "$0")"
if [ -z "$archive_line" ]; then echo 'SimLab archive header is invalid' >&2; exit 1; fi

data_root="${XDG_DATA_HOME:-$HOME/.local/share}/SimLab/versions"
destination="$data_root/$version-linux-$architecture-${expected_sha%????????????????????????????????}"
mkdir -p "$data_root"
valid_install() {
  test -f "$1/.complete" && test -f "$1/.files.sha256" || return 1
  index_hash="$(sha256sum "$1/.files.sha256" | cut -d ' ' -f 1)"
  test "$index_hash" = "$expected_index" || return 1
  (cd "$1" && sha256sum --check --status .files.sha256)
}
if ! valid_install "$destination"; then
  lock="$destination.lock"
  if ! mkdir "$lock" 2>/dev/null; then echo 'Another SimLab extraction is in progress; retry after it completes' >&2; exit 1; fi
  work="$(mktemp -d "$data_root/.extract.XXXXXX")"
  cleanup() { rm -rf "$work"; rmdir "$lock"; }
  trap cleanup EXIT
  trap 'exit 130' HUP INT
  trap 'exit 143' TERM
  for process in /proc/[0-9]*; do
    entry="$(readlink "$process/exe" 2>/dev/null || true)"
    case "$entry" in "$destination"/*) echo 'Close all SimLab windows before repairing this installation' >&2; exit 1 ;; esac
  done
  tail -n "+$archive_line" "$0" > "$work/archive.tar.gz"
  actual_sha="$(sha256sum "$work/archive.tar.gz" | cut -d ' ' -f 1)"
  if [ "$actual_sha" != "$expected_sha" ]; then echo 'SimLab archive checksum failed' >&2; exit 1; fi
  tar -xzf "$work/archive.tar.gz" -C "$work"
  test -x "$work/SimLab/run-simlab.sh"
  touch "$work/SimLab/.complete"
  valid_install "$work/SimLab"
  backup="$destination.damaged.$$"
  if [ -d "$destination" ]; then mv -T "$destination" "$backup"; fi
  if ! mv -T "$work/SimLab" "$destination"; then
    if [ -d "$backup" ]; then mv -T "$backup" "$destination"; fi
    echo 'Could not switch to the repaired SimLab payload; old files were preserved' >&2
    exit 1
  fi
  cleanup
  trap - EXIT HUP INT TERM
fi
if [ "${1:-}" = --extract-only ]; then printf '%s\n' "$destination"; exit 0; fi
exec "$destination/run-simlab.sh" "$@"
exit 1
__SIMLAB_ARCHIVE_BELOW__
