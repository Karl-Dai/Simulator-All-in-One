# Shared versions and portable Windows packages

Implemented on 2026-11-06. These changes are being validated on the Windows acceptance branch and are unreleased.

All eight bundled simulators use `components.lock.json.suiteVersion`. Component entries retain `sourceVersion` for upstream provenance, alongside the pinned source commit and patch SHA-256. Those source versions do not determine application versions or update behavior. Standalone upstream checkouts remain unchanged.

The suite builder injects the version into Tauri's merged configuration, and into both Wails metadata and `main.Version` for Modbus. Build receipts include the suite version; collecting a previous version's payloads fails. Manifest validation requires each component version to match the suite version and verifies its source and patch identity.

Windows packages are named `SAIO_<version>_windows-amd64-portable.exe`. A small Go entrypoint embeds the launcher, eight applications and their resources as a verified archive. The entrypoint extracts to `%LOCALAPPDATA%\SAIO\portable\<payload-sha256>` and reuses verified files. A damaged cache is moved aside before replacement. User data remains in `%APPDATA%\SimLab`; extraction and repair never remove projects. Different payloads use different directories, so an existing running copy is not overwritten.

WebView2 is a separate prerequisite. The Windows launcher checks it before creating any webview and offers a native download prompt if it is missing or below Wails' required version. Accepting opens the download page and exits; canceling exits without opening a browser. The program does not download or install the runtime automatically.

The live prerequisite page is <https://simlab.carldai.cloud/downloads/webview2/>. Its x64 offline installer has SHA-256 `0625fb5c1e75be58a164e14f95aa58a3e5badb859b3d17c952385f3e4a9ff3e8`. Nginx serves these files independently of the SimLab application container from `/var/www/simlab-downloads/webview2/`. The previous production vhost is backed up as `/etc/nginx/sites-available/simlab.carldai.cloud.before-webview-20261106-231218`; the matching local SimLab vhost configuration contains the download route.

Validation:

- Rebuilt all eight macOS ARM applications and the launcher using the pinned tools. The packaged applications report version `0.1.4`, and the native launcher verifies all eight roles.
- Python packaging tests: 71 passed. Launcher frontend tests: 53 passed. Go race tests and workflow syntax checks passed.
- Cross-built the Windows launcher and a portable fixture EXE, including icon and version resources. Executed the same fixture extractor natively on macOS to verify extraction, cache reuse and repair.
- Checked the About screen in a headless browser using the native launcher's real snapshot as fixture data. Checked the live prerequisite download page and file availability.
- Windows native GUI, the missing-runtime dialog and all eight Windows applications have not been executed in this session. `scripts/test-windows-portable.ps1` performs extraction, cache repair, shared-version, data-preservation and process-startup checks on native Windows CI.

Evidence is in `.validation/unified-version-20261106/`. The existing GitHub v0.1.4 installers are unchanged; release preparation now requires the portable EXE for subsequent releases. The user authorized an independent acceptance branch and Windows CI build; no main-branch merge or Release is included.
