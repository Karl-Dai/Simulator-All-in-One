# Simulator All-in-One (SAIO)

English | [简体中文](README.zh-CN.md)

**One installer. Eight industrial protocol simulators. Independent windows.**

Simulator All-in-One, formerly **SimLab Desktop**, bundles IEC 60870-5-104, Modbus, PMU and OPC UA tools in one desktop suite. Open any simulator without downloading or installing individual components. Each runs in its own process and window, and keeps running when the launcher closes.

Repository: [Karl-Dai/Simulator-All-in-One](https://github.com/Karl-Dai/Simulator-All-in-One).

| Protocol | Included applications | Component version |
| --- | --- | --- |
| IEC 60870-5-104 | IEC104Master, IEC104Slave | 1.15.22 |
| Modbus | ModbusMaster, ModbusSlave | 0.17.9 |
| PMU V2 / V3 | PmuSim, PmuSub | 0.13.2 |
| OPC UA | OPCUAMaster, OPCUAServer | 0.8.0 |

## Features

- Eight bundled roles, ready to open from one launcher.
- Favorites and persistent appearance and language preferences.
- English by default on new installations, with Simplified Chinese and system-language options. Existing language choices are preserved.
- Separate suite data directories for each role, including OPC UA PKI and configuration.
- Manifest, SHA256, architecture and complete resource-tree checks before packaging.
- Suite-owned updates: bundled simulators do not independently update themselves. Automatic replacement of the complete suite is not implemented in version 0.1.3.

## Installation and current status

Download **v0.1.3** from [GitHub Releases](https://github.com/Karl-Dai/Simulator-All-in-One/releases/tag/v0.1.3). Packages use these names:

| Target | Installer | Launcher |
| --- | --- | --- |
| macOS Apple Silicon, 13.0+ | `SAIO_0.1.3_darwin-arm64.dmg` | `Simulator All-in-One.app` |
| macOS Intel, 13.0+ | `SAIO_0.1.3_darwin-amd64.dmg` | `Simulator All-in-One.app` |
| Windows x64 | `SAIO_0.1.3_windows-amd64-setup.exe` | `SAIO.exe` |
| Ubuntu 22.04 x64 baseline | `SAIO_0.1.3_linux-amd64.run` | `SAIO` |

The public release contains six archives built from the same commit: two macOS DMGs, two macOS ZIPs, one Windows installer and one Linux archive. Verify downloads against `SHA256SUMS` or the adjacent `.sha256.json` files. [Four-target build and checks](https://github.com/Karl-Dai/Simulator-All-in-One/actions/runs/41967403951) passed; [publication](https://github.com/Karl-Dai/Simulator-All-in-One/actions/runs/41974642759) verifies source provenance and all package checksums before release.

This release passed native compilation and packaging for all four targets, Windows installation/repair/startup/uninstall checks, and Linux offline container startup. Earlier macOS Apple Silicon builds received actual window and launcher interaction checks; those historical checks do not establish complete GUI or protocol acceptance for this release. Full Windows/Linux/Intel GUI and protocol acceptance, a fresh Windows offline installation without WebView2, and macOS 13 hardware validation remain open. See the [dated implementation and acceptance record](docs/implementation-status.md).

On macOS, drag `Simulator All-in-One.app` from the new DMG into Applications. Current packaging supports ad-hoc signing or an existing signing identity; Developer ID distribution and Apple notarization remain pending.

The Windows NSIS installer bundles all eight payloads and a pinned, SHA256-verified offline WebView2 Standalone Installer. It installs WebView2 only when missing, blocks replacement while suite applications are running, repairs declared files, and refuses to overwrite unrelated files.

The Linux `.run` archive verifies and extracts to a stable directory under `${XDG_DATA_HOME:-$HOME/.local/share}/SimLab/versions/`, then starts the launcher. It includes GTK3/WebKitGTK 4.1 runtime files and does not require FUSE. The host must still provide glibc 2.35+, a display server and compatible graphics/EGL/GLX drivers.

## Data and rename compatibility

The public product name, About screens, repository and new executable/package names use **Simulator All-in-One / SAIO**. The Go module is `github.com/Karl-Dai/Simulator-All-in-One`.

Compatibility identities remain unchanged: the `SimLab` user-data directory, `SIMLAB_*` environment variables, `cloud.carldai.simlab*` bundle IDs, `simlab-suite` build feature, Windows installation/uninstall registry paths, and `.simlab-owned` marker. Existing projects, certificates, favorites and saved language choices continue to use their current locations. The Windows checker accepts both the legacy `SimLab.exe` and the new `SAIO.exe`; new installers replace the legacy Start Menu shortcut.

Historical records and existing artifacts retain their original names. Original standalone simulator repositories are not modified by suite builds; exact upstream commits and suite patches are used in isolated component checkouts.

## Build from source

Build tools are pinned in [`toolchain.lock.json`](toolchain.lock.json). Use the selected Python interpreter to check your environment before building:

```sh
python scripts/toolchain.py check
```

The checker reports missing or mismatched tools without changing your installation. CI reads the same version file and remains manual-only. Build receipts and the suite manifest retain the observed tool versions, native environment details and configuration digest; a changed toolchain requires a new native build. The selected versions are a unified candidate until all four native targets have been revalidated.

Shared appearance contracts and Vue components live in `ui/`. ModbusMaster and IEC104Master are the initial integration targets; the remaining roles retain the legacy theme entry point. Suite language changes apply to the current session without overwriting standalone language preferences. Shared files are exported and checked by `scripts/sync_ui.py`; edit the source in `ui/` and regenerate the reviewed suite patches rather than editing generated copies.

Use the exact build-tool versions in `toolchain.lock.json`; the bootstrap scripts themselves require Python 3.10+. macOS requires Xcode Command Line Tools; Windows requires Visual Studio C++ tools, Git Bash and NSIS; Linux requires GTK3/WebKitGTK 4.1 development packages. The Wails CLI remains pinned to **v2.14.0**.

```sh
git clone https://github.com/Karl-Dai/Simulator-All-in-One.git
cd Simulator-All-in-One
go install github.com/wailsapp/wails/v2/cmd/wails@v2.14.0
python3 scripts/suite.py bootstrap
python3 scripts/suite.py build --platform darwin --arch arm64
python3 scripts/suite.py collect --platform darwin --arch arm64
python3 scripts/suite.py verify --platform darwin --arch arm64
python3 scripts/suite.py package --platform darwin --arch arm64
```

Build each target on its native platform and architecture. Replace `--platform` / `--arch` with `darwin` / `amd64`, `windows` / `amd64` or `linux` / `amd64` as appropriate. On Linux, run runtime collection and audit before packaging:

```sh
python3 packaging/linux/runtime.py prepare dist/staging/linux-amd64
python3 packaging/linux/runtime.py audit dist/staging/linux-amd64
```

`components.lock.json` pins all four upstream commits, component versions, suite patches and Rust lockfiles. npm builds use lockfiles and `npm ci`; Rust builds use `--locked`. `bootstrap` refuses dirty component checkouts rather than resetting them. For another workspace, use the same absolute `--workspace` path for every command. Do not bootstrap an existing manually adapted `.components/` tree again.

`collect` requires all eight roles and valid source/build receipts. macOS children are signed before their final hashes are written, and the outer app is signed separately. `SIMLAB_SIGNING_IDENTITY` or `--signing-identity` selects an existing identity; notarization is not part of this pipeline. Byte-for-byte reproducibility across compilers or OS images is not promised.

```text
dist/staging/darwin-arm64/Simulator All-in-One.app/Contents/
  Helpers/<Product>.app
  Resources/suite-manifest.json

dist/staging/windows-amd64/
  SAIO.exe
  suite-manifest.json
  apps/<role>/

dist/staging/linux-amd64/
  SAIO
  suite-manifest.json
  apps/<role>/
  runtime/
  run-simlab.sh
```

## Checks and CI

```sh
go test -race ./internal/suite
go vet ./internal/suite
python3 -m unittest discover -s scripts/tests -v
python3 -m py_compile scripts/*.py packaging/linux/runtime.py
sh -n packaging/linux/run-simlab.sh packaging/linux/self-extract.sh
actionlint .github/workflows/*.yml
cd frontend
npm ci
npm test
npm run build
```

All workflows are started manually (`workflow_dispatch`); validation and native builds can also be called by the release workflow. Pushes, pull requests, tags and schedules do not start CI. After pushing an annotated version tag, run `release.yml` on that tag to validate, build all four targets and publish their packages.

`validate.yml` checks the launcher and packaging. `build-suite.yml` builds macOS ARM/Intel, Windows x64 and Ubuntu 22.04 x64, verifies complete bundles, uploads artifacts and runs available native startup/installer checks. `validate-package.yml` only reuses trusted compatible packages; changes to compiled inputs require a full rebuild. An uploaded artifact or surviving process alone does not prove complete GUI or protocol acceptance.

See [implementation and acceptance evidence](docs/implementation-status.md) and the [Chinese build and packaging guide](README.zh-CN.md) for detailed platform constraints, pinned offline WebView2 distribution and runtime closure handling.

## Changelog

See [CHANGELOG.md](CHANGELOG.md) and [GitHub Releases](https://github.com/Karl-Dai/Simulator-All-in-One/releases).
