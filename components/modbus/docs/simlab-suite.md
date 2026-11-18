# SimLab embedded builds

The standalone commands and `wails.json` files keep their existing behavior.
The suite script merges the small `wails.suite.json` overlay for the build and
restores the original config and macOS plist on exit. Do not run a standalone
build of the same role concurrently with a suite build.

From the repository root, with Go, Node.js and native Wails prerequisites:

```sh
npm ci --ignore-scripts
./go/scripts/suite.sh master -platform darwin/arm64
./go/scripts/suite.sh slave -platform darwin/arm64
# On a Linux host with GTK3 and WebKitGTK 4.1 development packages:
./go/scripts/suite.sh master -platform linux/amd64
./go/scripts/suite.sh slave -platform linux/amd64
# On a Windows host (Git Bash):
./go/scripts/suite.sh master -platform windows/amd64 -webview2 error
./go/scripts/suite.sh slave -platform windows/amd64 -webview2 error
```

The script always enables `simlab_suite`; Linux also enables `webkit2_41`.
It emits the normal `go/cmd/{master,slave}gui/build/bin` payloads, with the
macOS bundle IDs `cloud.carldai.simlab.modbus.master` and
`cloud.carldai.simlab.modbus.slave`. No child installer is needed on Windows;
the suite owns the WebView2 prerequisite and the complete suite update.

The launcher supplies an absolute `SIMLAB_DATA_DIR` (its user configuration
directory plus `SimLab`). Each role owns `modbus.master` or `modbus.slave`
below it. A missing override defaults to `os.UserConfigDir()/SimLab`; relative
overrides fail explicitly. Windows places its WebView2 profile there; Linux
sets per-role XDG data/config/cache directories before Wails initializes.
macOS WKWebView storage uses the separate suite bundle ID in the standard
macOS website-data location. User-selected `.modbusproj` and certificate paths
retain their existing semantics and are never rewritten by suite startup.

Checks (apply the normal macOS CGO flags when testing on macOS):

```sh
./go/scripts/mastergui.sh test ./...
./go/scripts/mastergui.sh test -tags simlab_suite ./...
```
