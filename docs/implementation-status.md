# 实施与验收记录

初版记录日期：2026-10-22。套件版本：0.1.0。本文记录初版实施、本地验收及已获授权的私有仓库交付和三平台 CI；2026-10-30 的界面更新与重打包记录见文末。正式发布不属于本轮范围。

## 已实现

- 一个 Go/Wails 2 + Vue 3 桌面入口，四组协议、八个角色直接启动，收藏、最近使用、系统/浅色/深色主题。
- 八个内置应用的套件构建配置，独立应用标识和数据路径；套件模式禁用独立更新和 pending update。
- OPC UA 证书管理与协议运行时的 PKI 路径一致，配置写入用户目录。
- 固定四个上游提交及补丁、Rust 锁文件、Windows 离线 WebView2 安装器的 URL/哈希。
- 三个平台的完整组装与打包脚本、完整资源树检查、安装目录保护及修复处理。
- GitHub 原生构建工作流：macOS arm64/amd64、Windows amd64、Linux amd64；已在私有仓库启动，可按目标平台单独重跑。

原有开发仓库保持不变。适配源位于忽略跟踪的 `.components/`，可审阅和复用的变更保存在 `patches/`。

## 实际验证

| 项目 | 本次结果 |
| --- | --- |
| macOS Apple Silicon 构建 | 主入口和八个完整子 `.app` 构建通过 |
| 完整安装包 | DMG/ZIP 已生成；DMG 校验、8/8 清单及完整资源树、嵌套 ad-hoc 签名验证通过 |
| 主入口按钮 | 从真实主窗口逐个点击八个角色，观察到对应可操作的原生窗口及九个独立进程 |
| 离线启动 | 使用继承式进程 sandbox 禁止网络，先确认网络调用返回 EPERM；全新测试数据目录中八个角色均直接打开并完整渲染 |
| 主入口退出 | 主入口退出后八个子进程继续运行；PMU 约 51 帧/秒、OPC UA 连接保持 |
| Modbus | 内置主站连接本机内置从站；通过 TCP 完成 FC06 写 42、FC03 读回 42，并恢复 0 |
| IEC104 | 内置子站在本机响应 STARTDT、总召激活确认、七类监视 ASDU 和总召结束；主站窗口已验证，主站 GUI 总召路径未单独验收 |
| PMU | 内置主/子站 V3 握手及约 50 帧/秒连续接收通过 |
| OPC UA | 内置客户端连接本机内置服务端；服务端建立验证变量；PKI 位于套件用户数据目录，安装树无新建 pki 目录 |
| 安装后启动 | 从 DMG 安装到 `/Applications/SimLab.app`，异地 8/8 验证、签名验证、原生主窗口渲染及包内 Modbus 启动通过 |
| 自动化检查 | Go race/vet、5 项入口交互测试、16 项打包与检查脚本测试、Python/shell 语法、actionlint、前端生产构建和 npm audit 通过；各子项目的相关测试由适配代理执行通过 |

测试原始记录保存在本机忽略跟踪的 `.validation/`。离线检查是在当前 macOS 主机做的进程网络隔离，不等同于另一台全新机器的系统安装验收。

## GitHub 验证

仓库：[Karl-Dai/SimLab-Desktop](https://github.com/Karl-Dai/SimLab-Desktop)，私有。源码与锁文件已提交推送；构建产物不提交进 Git。

| 目标 | 原生构建与打包 | 运行验收与产物 |
| --- | --- | --- |
| macOS ARM | [39766494441](https://github.com/Karl-Dai/SimLab-Desktop/actions/runs/39766494441)，`858a04a`，通过 | [DMG / ZIP](https://github.com/Karl-Dai/SimLab-Desktop/actions/runs/39766494441/artifacts/12420289226)；本机另外复核哈希、完整资源树、签名、DMG 和入口 `--verify` 8/8。真实窗口及协议操作见上表 |
| macOS Intel | [39767223585](https://github.com/Karl-Dai/SimLab-Desktop/actions/runs/39767223585)，`858a04a`，通过 | [DMG / ZIP](https://github.com/Karl-Dai/SimLab-Desktop/actions/runs/39767223585/artifacts/12423911828)；本机复核哈希、完整资源树、九个 x86_64 二进制、嵌套签名和 DMG；未执行 Intel 代码 |
| Linux x64 | [39780973379](https://github.com/Karl-Dai/SimLab-Desktop/actions/runs/39780973379)，`bca65d6`，八角色、入口、后端、3208 文件闭包及 `.run` 打包通过 | [离线复验 39786221290](https://github.com/Karl-Dai/SimLab-Desktop/actions/runs/39786221290)，`ddc14a7`，通过；[完整 `.run` 与来源记录](https://github.com/Karl-Dai/SimLab-Desktop/actions/runs/39786221290/artifacts/12429376456) |
| Windows x64 | [39772981768](https://github.com/Karl-Dai/SimLab-Desktop/actions/runs/39772981768)，`90ac9c4`，八角色、入口、后端、完整清单及 NSIS 打包通过 | [修复后复验 39786938061](https://github.com/Karl-Dai/SimLab-Desktop/actions/runs/39786938061)，`bd7e3e7`，全部通过；[完整 NSIS 安装包与来源记录](https://github.com/Karl-Dai/SimLab-Desktop/actions/runs/39786938061/artifacts/12429287605)。覆盖安装、缺文件修复、未知文件保护、活动实例检查、九个进程启动及卸载 |

Windows 原生校验在 32/64 位 Windows PowerShell 5.1、继承和默认模块环境四种组合中通过。之前出现 `Get-FileHash` 未能加载，已在校验器进程内显式加载本机 PowerShell 自带模块；真实安装事务随后通过。本机只对 Windows 包做静态解包与校验：九个 amd64 PE、8/8 完整资源树、内置校验脚本和离线 WebView2 锁文件均匹配，没有执行 Windows 二进制。最终安装包 SHA256 为 `e8ef142a8063c1b3a6b48e6ceb3cd6202d1abe91169491bb5016c938fab5894e`。

Linux 离线复验使用同一个 `bca65d6` 原包，SHA256 为 `b21d26a101639f03b716eedf68fd23baeeb1e678988f840ec947a4416c614a53`。宿主仅提供声明的 glibc、显示及图形库，没有安装 GTK/WebKit，容器使用 `--network none`。通过进程启动检查不等同于完整 GUI 或协议验收。

前几轮原生 CI 暴露的 Git Bash 选择、Xcode `lipo` 参数、Windows cp1252 文本解码和 Wayland UI 依赖收集问题已修复；Linux 校验镜像补齐声明的宿主 GLES 图形库。旧 Linux 任务曾长时间无日志，已改为独立且有界的日志读取和阶段超时，未据后续缺库错误倒推出旧等待根因。

`validate-package.yml` 对原生包来源、祖先提交、允许改动范围、包哈希及八角色资源清单逐层核对，记录原始编译提交和复验提交。Windows 仅恢复原清单内的 payload 后重新打包；Linux 原包内容不变。调整编译输入或 Linux 运行时内容时禁止复用。构建工作流的工具链或编译参数变化仍需人工选择完整原生构建。

## 尚未完成

- Windows、Linux 和 macOS Intel 的完整窗口与协议操作验收。
- 无 WebView2 的干净 Windows 断网首次安装。
- macOS 最低版本 13.0 的实机验收；本轮本机系统为 macOS 27.0.1。
- Developer ID 签名、Apple 公证和正式发布。

安装包与 stage 内容的 37 个文件路径、哈希和权限一致。通过本机 CLI `open` 的一次启动出现黑屏及 WebKit/XPC 异常；停止该验证进程后，原生应用接口首次启动正常，主窗口显示 8/8 就绪。具体系统机制未确认，未通过修改资源或关闭保护规避。

本轮已验证安装包集中在本机 `dist/ci-packages/`，包含 macOS ARM/Intel DMG、Windows NSIS、Linux `.run`、校验值和 Windows/Linux 来源记录。私有仓库已交付，未创建 release 或 tag。

## 2026-10-30 界面更新与完整重建

入口布局调整 `7093ce4` 与双语界面 `b5371b8` 已推送到私有仓库 `main`。本次保持套件版本 0.1.0，组件版本及锁定上游提交不变；四个原生目标从同一源码提交 `b5371b80a0a59a7c8caac68bef5a8b3160621ef8` 完整构建。

- 入口改为顶栏导航；可选择中文、English 或跟随系统，语言偏好经 Wails API 保存。
- 本地入口后端 `go test -race ./internal/suite` / `go vet ./internal/suite`、7 项前端交互测试、16 项打包测试及前端生产构建通过。[源码 CI 40989673008](https://github.com/Karl-Dai/SimLab-Desktop/actions/runs/40989673008) 通过。
- 真实无头 Chromium 检查通过：中英切换、刷新恢复选项、收藏过滤、启动提示、主题与八组件关于页；1160×900 默认窗口八入口一屏可见，820×650 窗口无横向溢出，八入口均可滚动访问。此层使用明确的 Wails fixture，不代表原生进程或协议验收。
- 新 macOS ARM 包的实际原生窗口显示 8/8 就绪；切换英文后，`launcher.json` 写入 `language=en`，退出并重新打开后保持英文。从英文入口点击打开内置 ModbusMaster，真实子窗口完整渲染；关闭入口后子窗口继续运行。验收结束已关闭相关进程并恢复原入口偏好。本次未重做协议互联测试。

完整构建与验收任务：[40989672953](https://github.com/Karl-Dai/SimLab-Desktop/actions/runs/40989672953)，四个原生目标均通过。

| 目标 | 本轮 CI 与本机复核 | 完整产物 |
| --- | --- | --- |
| macOS ARM | 完整构建、后端 race 测试和打包通过；本机复核包哈希、八角色资源树、九个 arm64 程序、嵌套签名和 DMG，并完成上述真实入口 UI 验收 | [DMG / ZIP](https://github.com/Karl-Dai/SimLab-Desktop/actions/runs/40989672953/artifacts/12949515047) |
| macOS Intel | 完整构建、后端 race 测试和打包通过；本机复核包哈希、八角色资源树、九个 x86_64 程序、嵌套签名和 DMG，未在本机执行 Intel 程序 | [DMG / ZIP](https://github.com/Karl-Dai/SimLab-Desktop/actions/runs/40989672953/artifacts/12950596037) |
| Windows x64 | 完整构建、后端测试、NSIS 打包、安装、缺组件修复、未知文件保护、活动实例保护、九进程启动及卸载通过；本机仅静态解包，复核九个 x64 程序、八角色资源树和离线 WebView2 哈希 | [NSIS](https://github.com/Karl-Dai/SimLab-Desktop/actions/runs/40989672953/artifacts/12950140603) |
| Linux x64 | 完整构建、后端 race 测试、运行时闭包、打包和无宿主 GTK/WebKit 的断网容器 9/9 进程启动检查通过；本机静态解包复核 3,219 个索引文件、九个 x64 程序及八角色资源树 | [自解压 `.run`](https://github.com/Karl-Dai/SimLab-Desktop/actions/runs/40989672953/artifacts/12949610605) |

本机最新产物位于 `dist/ci-packages/b5371b8/`：四个目标的完整安装包、两个 macOS ZIP、各包 `.sha256.json`、总校验表 `SHA256SUMS`、四份 manifest 和 `provenance.json`。四个入口二进制均已确认嵌入新版双语界面及 `SetLanguage`；两个 macOS DMG 与各自 ZIP 的 37 项文件路径、哈希和权限一致。旧包保留，未覆盖本机 `/Applications/SimLab.app`。

原始构建状态、各目标解包检查、原生日志、浏览器截图和本机 UI 验收摘要保存在忽略跟踪的 `.validation/refresh-20261030/`。本次两笔已提交的入口改动与安装包已对齐；本条记录之后的文档提交不改变编译输入。工作区中另有并行的 UI 统一改动，未纳入本轮构建。

尚未完成项仍为 Windows/Linux/macOS Intel 的完整窗口与协议互联、无 WebView2 的干净 Windows 断网首装、macOS 13 实机、Developer ID 签名及 Apple 公证。本轮没有创建 tag 或 GitHub Release。
