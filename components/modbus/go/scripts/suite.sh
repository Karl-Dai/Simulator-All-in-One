#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
: "${SAIO_VERSION:?Build through SAIO scripts/suite.py to supply the suite version}"
role="${1:-}"
case "$role" in
  master|slave) shift ;;
  *) echo 'Usage: suite.sh master|slave [Wails build flags]' >&2; exit 2 ;;
esac
project_dir="$repo_root/go/cmd/${role}gui"
platform="$(go env GOOS)/$(go env GOARCH)"
expect_platform=0
for arg in "$@"; do
  if [[ "$expect_platform" == 1 ]]; then platform="$arg"; expect_platform=0; fi
  case "$arg" in
    -platform) expect_platform=1 ;;
    -platform=*) platform="${arg#-platform=}" ;;
  esac
done
if [[ "$expect_platform" == 1 ]]; then
  echo '-platform requires a value' >&2; exit 2
fi

# Wails 2 reads wails.json only. Restore the standalone config and plist even
# after a failed build, and prevent two suite builds of one role from racing.
lock_dir="$project_dir/.simlab-build.lock"
if ! mkdir "$lock_dir" 2>/dev/null; then
  echo "Another suite build owns $lock_dir" >&2; exit 1
fi
backup_dir="$(mktemp -d "${TMPDIR:-/tmp}/simlab-modbus-${role}.XXXXXX")"
cp "$project_dir/wails.json" "$backup_dir/wails.json"
cp "$project_dir/build/darwin/Info.plist" "$backup_dir/Info.plist"
cp "$project_dir/build/appicon.png" "$backup_dir/appicon.png"
if [[ -f "$project_dir/build/windows/icon.ico" ]]; then
  cp "$project_dir/build/windows/icon.ico" "$backup_dir/icon.ico"
fi
restore() {
  cp "$backup_dir/wails.json" "$project_dir/wails.json"
  cp "$backup_dir/Info.plist" "$project_dir/build/darwin/Info.plist"
  cp "$backup_dir/appicon.png" "$project_dir/build/appicon.png"
  if [[ -f "$backup_dir/icon.ico" ]]; then
    cp "$backup_dir/icon.ico" "$project_dir/build/windows/icon.ico"
  else
    rm -f "$project_dir/build/windows/icon.ico"
  fi
  rm -rf "$backup_dir"
  rmdir "$lock_dir"
}
trap restore EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

cp "$project_dir/suite-icons/icon.png" "$project_dir/build/appicon.png"
mkdir -p "$project_dir/build/windows"
cp "$project_dir/suite-icons/icon.ico" "$project_dir/build/windows/icon.ico"

node - "$project_dir" "$role" <<'NODE'
const fs = require('node:fs')
const path = require('node:path')
const [project, role] = process.argv.slice(2)
const configPath = path.join(project, 'wails.json')
const base = JSON.parse(fs.readFileSync(configPath, 'utf8'))
const suite = JSON.parse(fs.readFileSync(path.join(project, 'wails.suite.json'), 'utf8'))
if (!/^\d+\.\d+\.\d+$/.test(process.env.SAIO_VERSION || '')) throw new Error('invalid SAIO_VERSION')
suite.info = { ...suite.info, productVersion: process.env.SAIO_VERSION }
fs.writeFileSync(configPath, JSON.stringify({ ...base, ...suite, info: { ...base.info, ...suite.info } }, null, 2) + '\n')
const plistPath = path.join(project, 'build/darwin/Info.plist')
const original = fs.readFileSync(plistPath, 'utf8')
const standaloneId = `com.modbus${role}.dev`
if (!original.includes(`<string>${standaloneId}</string>`)) throw new Error('unexpected standalone bundle identifier')
fs.writeFileSync(plistPath, original.replace(`<string>${standaloneId}</string>`, `<string>cloud.carldai.simlab.modbus.${role}</string>`))
NODE

if [[ "$platform" == darwin/* ]]; then
  export CGO_ENABLED=1
  export CGO_CFLAGS="${CGO_CFLAGS:+$CGO_CFLAGS }-mmacosx-version-min=13.0"
  export CGO_LDFLAGS="${CGO_LDFLAGS:+$CGO_LDFLAGS }-framework UniformTypeIdentifiers -mmacosx-version-min=13.0"
fi
tags=simlab_suite
if [[ "$platform" == linux/* ]]; then tags="$tags,webkit2_41"; fi

wails_binary="$(command -v wails || true)"
if [[ -z "$wails_binary" ]]; then
  go_path="$(go env GOPATH)"
  wails_binary="${go_path%%:*}/bin/wails"
fi
wails_command=(go run github.com/wailsapp/wails/v2/cmd/wails@v2.14.0)
if [[ -x "$wails_binary" ]]; then
  wails_version="$("$wails_binary" version)"
  if [[ "${wails_version%%$'\n'*}" == v2.14.0 ]]; then wails_command=("$wails_binary"); fi
fi
cd "$project_dir"
"${wails_command[@]}" build -tags "$tags" -ldflags "-X main.Version=$SAIO_VERSION" "$@"
