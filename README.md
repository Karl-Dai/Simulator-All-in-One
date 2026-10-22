# SimLab

一个安装包，内置八个工业协议模拟器。安装后从统一入口点击打开，各模拟器使用独立窗口和进程；关闭入口后，已经打开的模拟器继续运行。用户无需分别下载或安装八个程序。

| 协议 | 内置程序 | 当前组件版本 |
|---|---|---|
| IEC 60870-5-104 | IEC104Master、IEC104Slave | 1.15.22 |
| Modbus | ModbusMaster、ModbusSlave | 0.17.9 |
| PMU V2 / V3 | PmuSim、PmuSub | 0.13.2 |
| OPC UA | OPCUAMaster、OPCUAServer | 0.8.0 |

当前套件版本为 **0.1.0**。本地已生成 macOS Apple Silicon 完整 DMG / ZIP，验证八个组件、清单哈希、资源树和嵌套签名；macOS Intel、Windows 和 Linux 的原生构建/安装验收尚未完成。构建工作流已提供，尚未在 GitHub 执行。当前 macOS 产物使用 ad-hoc 签名，未完成 Developer ID 签名或 Apple 公证。

## 安装与使用

macOS 要求 **13.0+**。打开 `SimLab_0.1.0_darwin-arm64.dmg`，将 `SimLab.app` 拖入“应用程序”。第一次打开若被系统拦截，可在“系统设置 → 隐私与安全性”确认打开。当前本地产物位于 `dist/packages/`，旁边的 `.sha256.json` 提供校验值；这些产物尚未发布到 GitHub Releases。

Windows 设计为一个 NSIS 安装包，内置八个免安装 payload，并携带一份对应架构的 WebView2 离线 Standalone Installer。安装器检测已有 WebView2，仅在缺少时安装；不会逐一启动八个子安装器。套件运行中禁止替换，完整新目录先准备并校验，再切换，切换失败保留或恢复旧目录。非空目标目录必须有 SimLab 所有权标记及可识别文件清单，未知文件会阻止覆盖；声明内程序缺失或损坏可以重装修复。该流程尚未在本机 Windows 实测。

Linux 目标基线为 **Ubuntu 22.04 x64**，产物为一个 `.run` 自解压包，包含 GTK3 / WebKitGTK 4.1 运行时。第一次运行解压到 `${XDG_DATA_HOME:-$HOME/.local/share}/SimLab/versions/` 下的稳定版本目录，再启动入口；不依赖 FUSE 或每次随机临时目录。已有目录先核对全文件清单，损坏时保留旧目录后重新铺设，活动实例会阻止修复。宿主仍提供 glibc 2.35+、显示服务以及匹配的显卡/EGL/GLX 驱动。运行时闭包审计和干净主机离线启动验收是两层检查，本机尚无 Linux 原生验收结果。

## 数据与更新

入口和子程序使用独立于原独立版的套件身份。启动器传递绝对 `SIMLAB_DATA_DIR`，默认位于系统用户配置目录下的 `SimLab`；组件按角色分目录。macOS WKWebView 网站数据使用各自的套件 bundle ID，Windows WebView2、Linux XDG 数据路径采用组件隔离目录。OPC UA 的 PKI 等可写资源保存在用户数据目录。工程文件及用户证书路径保留原应用语义，不向安装目录写配置。

套件子程序关闭独立更新及相应下载入口，版本随整个套件交付。首版未实现自动下载并替换套件。用户选择的工程、证书和连接参数不会因为关闭主入口而被删除。

## 源码锁与可重复构建

`components.lock.json` 锁定四个上游提交、组件版本、角色、构建位置及补丁：

| 上游 | 锁定提交 |
|---|---|
| [ModbusSim](https://github.com/Karl-Dai/ModbusSim) | `cb7654b0b1781abd342fae65db3ed8aead388c0d` |
| [IEC104](https://github.com/Karl-Dai/IEC60870-5-104-Simulator) | `d9355e9dadfa8d6ab7b0d0666788bc7fa745a6cd` |
| [PmuSim](https://github.com/Karl-Dai/PmuSim) | `483661acf89dee3362a97928c78f7e1a2ca8a9f9` |
| [OPCUASim](https://github.com/Karl-Dai/OPCUASim) | `d05cf5ef398c55955862e71874faa7321f31e87e` |

三个 Rust workspace 的依赖锁快照位于 `packaging/locks/`，校验值写入组件锁；构建使用 `--locked`。前端使用各仓库的 npm lockfile 和 `npm ci`。Wails CLI 固定为 **v2.14.0**。构建步骤可重复执行，未承诺不同编译器或系统镜像之间的产物字节完全相同。

需要 Python 3.10+、Git、Node.js 22、Go 1.27+、Rust/Cargo，以及对应平台的原生构建工具。macOS 需要 Xcode Command Line Tools；Windows 需要 Visual Studio C++ 工具、Git Bash、NSIS 和已校验的 WebView2 离线安装器；Linux 需要 GTK3 / WebKitGTK 4.1 开发包和运行时资源。

在全新检出中执行：

```sh
go install github.com/wailsapp/wails/v2/cmd/wails@v2.14.0
python3 scripts/suite.py bootstrap
python3 scripts/suite.py build --platform darwin --arch arm64
python3 scripts/suite.py collect --platform darwin --arch arm64
python3 scripts/suite.py verify --platform darwin --arch arm64
python3 scripts/suite.py package --platform darwin --arch arm64
```

`bootstrap` 浅拉取精确提交，只向干净、未适配的 checkout 严格应用补丁；不 reset、clean、覆盖未知脏源码。适配后记录补丁和源码指纹，后续构建发现外部修改即停止。本轮已有人工适配的 `.components` 快照不应再次 bootstrap；需要重新构建时，使用全新绝对 workspace，并给各命令传相同的 `--workspace /absolute/path`。

`build` 要求原生平台及架构，构建八个 payload 和入口，记录宿主来源/版本以及完整 payload 树的收据。`collect` 必须取得八个角色，核对收据、身份、架构和资源；缺少任一角色直接失败。`--adopt-existing` 仅用于本轮已审阅的 macOS 初始产物，要求上游 HEAD、整个补丁及子应用 bundle ID / 版本匹配，不是普通构建的默认旁路。

macOS 收集顺序为子应用内部签名 → 子应用签名 → 计算最终哈希 → 写 manifest → 单独签外层应用；外层签名不使用 `--deep` 重签子应用。默认 ad-hoc，`--signing-identity` / `SIMLAB_SIGNING_IDENTITY` 可提供构建机器上已有的身份；公证不包含在当前管线。

```text
dist/staging/darwin-arm64/SimLab.app/Contents/
  Helpers/<Product>.app
  Resources/suite-manifest.json

dist/staging/windows-amd64/
  SimLab.exe
  suite-manifest.json
  apps/<role>/<executable, DLLs and resources>

dist/staging/linux-amd64/
  SimLab
  suite-manifest.json
  apps/<role>/<executable and resources>
  lib/
  runtime/usr/
  runtime/closure.json
  run-simlab.sh
```

## Windows 与 Linux 打包

Windows 构建使用目标架构的 MSVC target 和静态 CRT。默认打包从 `webview2.lock.json` 的不可变微软 URL 获取 x64 离线 Standalone Installer，检查 SHA256 后内置；包体过小的 bootstrapper 会被拒绝。锁中的 `installerVersion` 是安装器自身文件版本，不是 WebView 渲染引擎版本。GitHub 工作流还校验 Microsoft Authenticode 签名。默认命令为 `python scripts/suite.py package --platform windows --arch amd64`；需要覆盖离线文件时，可按 [Microsoft 的离线分发说明](https://learn.microsoft.com/microsoft-edge/webview2/concepts/distribution#offline-deployment)准备文件并显式指定：

```sh
python scripts/suite.py package --platform windows --arch amd64 \
  --webview2 /absolute/path/MicrosoftEdgeWebView2RuntimeInstallerX64.exe \
  --webview2-sha256 <该离线文件的64位小写SHA256>
```

Linux 构建机固定为 Ubuntu 22.04 amd64；Wails 显式使用 `webkit2_41`。安装依赖可参考工作流的 apt 列表，然后执行：

```sh
python3 scripts/suite.py build --platform linux --arch amd64
python3 scripts/suite.py collect --platform linux --arch amd64
python3 packaging/linux/runtime.py prepare dist/staging/linux-amd64
python3 packaging/linux/runtime.py audit dist/staging/linux-amd64
python3 scripts/suite.py package --platform linux --arch amd64
```

运行时收集包括 WebKitWebProcess、WebKitNetworkProcess、注入 bundle、GTK/GIO/Pixbuf 模块、字体、所声明的沙箱工具和 GStreamer scanner，并递归收集 ELF 依赖。WebKit helper 路径采用 [Tauri 官方 GTK 打包器的等长重定位方法](https://github.com/tauri-apps/tauri/blob/dev/crates/tauri-bundler/src/bundle/linux/appimage/linuxdeploy-plugin-gtk.sh)，入口和子程序的 Linux cwd 固定为 `runtime/usr`；不依赖发行版忽略的 `WEBKIT_EXEC_PATH`，也不关闭 WebKit sandbox。相对外部文件路径因此以该目录解析，配置中应使用可访问的绝对证书路径。

缺少任何必要工具、运行时资源、闭包记录，存在未声明宿主库，或 glibc 要求高于基线时，打包明确失败。静态闭包不能证明所有运行行为，因此工作流另用没有安装 GTK/WebKit 的 Ubuntu 容器，在 `--network none` 下启动检查。

## 验证与工作流

```sh
python3 -m unittest discover -s scripts/tests -v
python3 -m py_compile scripts/*.py packaging/linux/runtime.py
sh -n packaging/linux/run-simlab.sh packaging/linux/self-extract.sh
actionlint .github/workflows/*.yml
```

本机已通过 8 项打包测试，覆盖脏源码保护、缺角色保留旧 stage、签名/哈希顺序、完整资源树、manifest 路径与哈希、Wails plist、自解压缺文件修复和旧目录保留。最终 macOS DMG 已通过 `hdiutil verify`，整套 `.app` 已通过 `codesign --verify --deep --strict`。

`validate.yml` 执行源码及打包静态检查；手动触发的 `build-suite.yml` 使用 macOS arm64 / x64、Windows x64、Ubuntu 22.04 x64 原生 runner，构建全部组件并上传完整套件。Windows job 默认读取 `packaging/windows/webview2.lock.json` 中固定的微软离线安装器 URL / SHA256；`WEBVIEW2_X64_URL` / `WEBVIEW2_X64_SHA256` 仓库变量可成对覆盖，校验不符直接失败，不降级为联网 bootstrapper。

Windows 工作流安排隔离目录安装、缺组件修复、未知文件保护、活动实例禁止替换、启动和卸载测试；runner 可能预装 WebView2，因此这仍不能替代“无 WebView2 的干净 Windows 断网首次安装”验收。Linux 工作流安排无宿主 GTK/WebKit 的断网容器启动检查。`smoke.py` 只验证进程启动存活和已知依赖错误，不代表完整 UI、真实设备或协议一致性验收。当前工作流尚未实际执行，Windows / Linux 不标为已通过。
