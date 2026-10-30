# SimLab

一个安装包，内置八个工业协议模拟器。安装后从统一入口点击打开，各模拟器使用独立窗口和进程；关闭入口后，已经打开的模拟器继续运行。用户无需分别下载或安装八个程序。

入口采用顶栏导航，提供收藏、最近使用、系统/浅色/深色主题，以及中文、English、跟随系统三种语言选项；主题与语言选择会保存并在下次启动时恢复。

| 协议 | 内置程序 | 当前组件版本 |
|---|---|---|
| IEC 60870-5-104 | IEC104Master、IEC104Slave | 1.15.22 |
| Modbus | ModbusMaster、ModbusSlave | 0.17.9 |
| PMU V2 / V3 | PmuSim、PmuSub | 0.13.2 |
| OPC UA | OPCUAMaster、OPCUAServer | 0.8.0 |

当前套件版本为 **0.1.0**。macOS Apple Silicon 和 Intel 的完整 DMG / ZIP 已通过原生 CI 构建、八个组件清单、资源树、嵌套签名及镜像校验；Apple Silicon 还完成了本机窗口与协议互联验收。Windows 已通过真实安装、修复、保护、九个进程启动及卸载检查；Linux 已通过干净容器离线启动检查。完整证据和验收边界见[实施记录](docs/implementation-status.md)。当前 macOS 产物使用 ad-hoc 签名，未完成 Developer ID 签名或 Apple 公证。

## 安装与使用

macOS 要求 **13.0+**。打开 `SimLab_0.1.0_darwin-arm64.dmg`，将 `SimLab.app` 拖入“应用程序”。第一次打开若被系统拦截，可在“系统设置 → 隐私与安全性”确认打开。2026-10-30 的双语入口完整重建基于 `b5371b8`，最新四个原生目标的安装包和两个 macOS ZIP 汇总在本机 `dist/ci-packages/b5371b8/`；`SHA256SUMS`、`.sha256.json` 和 `provenance.json` 提供校验值、来源提交、CI 与验收记录。旧包保留在 `dist/ci-packages/`。GitHub Actions 的下载链接见实施记录。这些产物尚未发布到 GitHub Releases。

Windows 使用一个 NSIS 安装包，内置八个免安装 payload，并携带一份对应架构的 WebView2 离线 Standalone Installer。安装器检测已有 WebView2，仅在缺少时安装；不会逐一启动八个子安装器。套件运行中禁止替换，完整新目录先准备并校验，再切换，切换失败保留或恢复旧目录。非空目标目录必须有 SimLab 所有权标记及可识别文件清单，未知文件会阻止覆盖；声明内程序缺失或损坏可以重装修复。该流程已在原生 Windows CI 通过安装、修复、目录保护、活动实例检查、入口与八角色启动和卸载；runner 可能预装 WebView2，尚未验证无 WebView2 的干净 Windows 断网首次安装。

Linux 目标基线为 **Ubuntu 22.04 x64**，产物为一个 `.run` 自解压包，包含 GTK3 / WebKitGTK 4.1 运行时。第一次运行解压到 `${XDG_DATA_HOME:-$HOME/.local/share}/SimLab/versions/` 下的稳定版本目录，再启动入口；不依赖 FUSE 或每次随机临时目录。已有目录先核对全文件清单，损坏时保留旧目录后重新铺设，活动实例会阻止修复。宿主仍提供 glibc 2.35+、显示服务以及匹配的显卡/EGL/GLX 驱动。运行时闭包审计和干净主机离线启动验收是两层检查；原生 CI 已通过 Ubuntu 22.04 无 GTK/WebKit、无网络容器中的入口与八角色进程启动检查，尚未验收完整窗口操作及协议互联。

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

本机已通过 16 项打包与检查脚本测试，覆盖脏源码保护、缺角色保留旧 stage、签名/哈希顺序、完整资源树、manifest 路径与哈希、Wails plist、自解压缺文件修复和旧目录保留、Windows 明确选择 Git Bash、cp1252 环境下的中文配置与资源收集，独立日志游标和有界读取、Wayland UI 依赖收集，以及原生包复用的来源与文件边界。macOS DMG 已通过 `hdiutil verify`，整套 `.app` 已通过 `codesign --verify --deep --strict`。

`validate.yml` 执行源码及打包静态检查；手动触发的 `build-suite.yml` 使用 macOS arm64 / x64、Windows x64、Ubuntu 22.04 x64 原生 runner，构建全部组件并上传完整套件。`target` 默认 `all`，也可选择一个原生目标，独立重跑失败的平台。Windows job 默认读取 `packaging/windows/webview2.lock.json` 中固定的微软离线安装器 URL / SHA256；`WEBVIEW2_X64_URL` / `WEBVIEW2_X64_SHA256` 仓库变量可成对覆盖，校验不符直接失败，不降级为联网 bootstrapper。

`validate-package.yml` 可对同一仓库已完成的 `main` 原生构建复验。脚本核对来源、祖先提交、安装包校验值和八角色完整清单；应用代码、补丁、锁文件或构建脚本变化会拒绝复用。Windows 只恢复清单内的原始 payload，并用当前安装脚本重新打包；Linux 仅复验原 `.run`，运行时内容变化必须完整重建。产物携带来源与本轮验证提交记录。复用沿用来源 run 的编译环境；调整构建工作流的编译参数或工具链后，应运行完整原生构建。

Rust 缓存保留失败任务的编译结果。完整包在运行验收之前上传，便于检查失败产物；可下载产物不代表该任务已通过。Linux 解压、虚拟显示与启动检查有阶段日志和超时限制。

Windows 工作流安排隔离目录安装、缺组件修复、未知文件保护、活动实例禁止替换、启动和卸载测试；runner 可能预装 WebView2，因此这仍不能替代“无 WebView2 的干净 Windows 断网首次安装”验收。Linux 工作流安排无宿主 GTK/WebKit 的断网容器启动检查。`smoke.py` 只验证进程启动存活和已知依赖错误，不代表完整 UI、真实设备或协议一致性验收。各平台结果在[实施记录](docs/implementation-status.md)中按实际证据记录。
