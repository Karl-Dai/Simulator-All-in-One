"""Build a single Windows EXE containing the complete suite, without a runtime installer."""
import json
import os
from pathlib import Path
import shutil
import tempfile
import zipfile


def build(stage, destination, version, root, run, digest):
    destination = destination.resolve()
    with tempfile.TemporaryDirectory(prefix="saio-portable-") as directory:
        work = Path(directory)
        for source in (root / "packaging/windows/portable").glob("*.go"):
            if not source.name.endswith("_test.go"):
                shutil.copy2(source, work / source.name)
        (work / "go.mod").write_text("module saio-portable\n\ngo 1.25.0\n")
        index = {}
        with zipfile.ZipFile(work / "payload.zip", "w", zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
            for path in sorted(stage.rglob("*")):
                if path.is_symlink():
                    raise ValueError(f"Portable payload must not contain links: {path}")
                if path.is_file():
                    name = path.relative_to(stage).as_posix()
                    if name.startswith("webview2/") or path.name.lower() in {"webview2runtime.exe", "uninstall.exe"}:
                        raise ValueError(f"Portable payload must not contain installers or a bundled WebView2: {path}")
                    archive.write(path, name)
                    index[name] = {"sha256": digest(path), "size": path.stat().st_size}
        (work / "payload-index.json").write_text(json.dumps(index, sort_keys=True) + "\n")
        (work / "payload.go").write_text('''package main
import _ "embed"
//go:embed payload.zip
var embeddedPayload []byte
//go:embed payload-index.json
var embeddedIndex []byte
func init() { payload = embeddedPayload; indexJSON = embeddedIndex }
''')
        shutil.copy2(root / "build/windows/icon.ico", work / "icon.ico")
        manifest = (root / "build/windows/wails.exe.manifest").read_text(encoding="utf-8")
        manifest = manifest.replace("{{.Name}}", "SAIOPortable").replace("{{.Info.ProductVersion}}", version)
        (work / "portable.manifest").write_text(manifest, encoding="utf-8")
        numeric = version.replace(".", ",") + ",0"
        (work / "portable.rc").write_text(f'''1 ICON "icon.ico"
1 24 "portable.manifest"
1 VERSIONINFO
 FILEVERSION {numeric}
 PRODUCTVERSION {numeric}
 FILEOS 0x40004
 FILETYPE 0x1
BEGIN
 BLOCK "StringFileInfo"
 BEGIN
  BLOCK "040904b0"
  BEGIN
   VALUE "CompanyName", "Karl-Dai\\0"
   VALUE "FileDescription", "Simulator All-in-One Portable\\0"
   VALUE "FileVersion", "{version}\\0"
   VALUE "ProductName", "Simulator All-in-One\\0"
   VALUE "ProductVersion", "{version}\\0"
   VALUE "OriginalFilename", "SAIO-portable.exe\\0"
  END
 END
 BLOCK "VarFileInfo"
 BEGIN
  VALUE "Translation", 0x409, 1200
 END
END
''')
        # Keep rc.exe for resource compilation, but use LLVM's COFF converter:
        # MSVC cvtres can emit absolute symbols rejected by Go's linker.
        if os.name == "nt":
            run(["rc.exe", "/nologo", "/fo", "portable.res", "portable.rc"], cwd=work)
            run(["llvm-cvtres.exe", "/MACHINE:X64", "/OUT:resource_windows_amd64.syso", "portable.res"], cwd=work)
        else:
            run(["x86_64-w64-mingw32-windres", "portable.rc", "-O", "coff", "-o", "resource_windows_amd64.syso"], cwd=work)
        env = dict(os.environ, GOOS="windows", GOARCH="amd64", CGO_ENABLED="0")
        run(["go", "build", "-trimpath", "-ldflags", "-s -w -H=windowsgui", "-o", destination, "."], cwd=work, env=env)
