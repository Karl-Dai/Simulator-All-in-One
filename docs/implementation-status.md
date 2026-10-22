# 实施与验收记录

日期：2026-10-22。套件版本：0.1.0。本文记录初版实施、本地验收及已获授权的私有仓库交付和三平台 CI。正式发布不属于本轮范围。

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
| 自动化检查 | Go race/vet、5 项入口交互测试、13 项打包与检查脚本测试、Python/shell 语法、actionlint、前端生产构建和 npm audit 通过；各子项目的相关测试由适配代理执行通过 |

测试原始记录保存在本机忽略跟踪的 `.validation/`。离线检查是在当前 macOS 主机做的进程网络隔离，不等同于另一台全新机器的系统安装验收。

## GitHub 验证

仓库：[Karl-Dai/SimLab-Desktop](https://github.com/Karl-Dai/SimLab-Desktop)，私有。源码与锁文件已提交推送；构建产物不提交进 Git。

- [源码及打包静态检查](https://github.com/Karl-Dai/SimLab-Desktop/actions/runs/39773936556)：提交 `04d26a3`，通过。
- [首轮完整原生矩阵](https://github.com/Karl-Dai/SimLab-Desktop/actions/runs/39765082475)：提交 `91eecb9`。Windows 构建遇到系统 WSL shim 被误作 Bash，已改为绝对 Git Bash 路径；macOS ARM 和 Intel 均已构建全部八个组件并通过后端测试，在收集时遇到 Xcode 16.4 `lipo -verify_arch` 参数解析错误，已将输入文件移到架构列表之前。Linux 已完成八个组件和入口构建、后端测试、完整清单、运行时闭包及 `.run` 打包；验证镜像准备成功后长期没有启动检查输出，已停止旧的无超时检查任务，完整日志保存在本机 `.validation/`；旧任务未证明断网启动成功。
- [Windows 首次单目标重跑](https://github.com/Karl-Dai/SimLab-Desktop/actions/runs/39766065716)：提交 `41907c0`。已构建 Modbus、IEC104、PMU 六个角色，在 PMU 子站构建后的资源收集中因 cp1252 默认编码读取 UTF-8 配置失败。已统一 Python/PowerShell 文本编码，并增加中文配置、资源的回归测试。
- [macOS ARM 单目标重跑](https://github.com/Karl-Dai/SimLab-Desktop/actions/runs/39766494441)：提交 `858a04a`，全部通过。八个角色构建、后端测试、完整清单、嵌套签名和 DMG 校验通过；[完整 DMG / ZIP 产物](https://github.com/Karl-Dai/SimLab-Desktop/actions/runs/39766494441/artifacts/12420289226)已上传。本机下载后另行复核包哈希、完整资源树、签名、DMG，以及 CI 编译的入口 `--verify` 报告 8/8 就绪。此 job 未做图形界面操作验收。
- [macOS Intel 单目标验证](https://github.com/Karl-Dai/SimLab-Desktop/actions/runs/39767223585)：提交 `858a04a`，全部通过。八个组件、后端测试、清单、签名、DMG/ZIP 打包与镜像校验通过；[完整产物](https://github.com/Karl-Dai/SimLab-Desktop/actions/runs/39767223585/artifacts/12423911828)已上传。本机下载后另行复核包哈希、完整资源树、全部九个 x86_64 二进制、嵌套签名与 DMG；未执行 Intel 代码或做 Intel 图形界面验收。
- [Windows UTF-8 修复重跑](https://github.com/Karl-Dai/SimLab-Desktop/actions/runs/39772981768)：提交 `90ac9c4`，进行中。
- [Linux 有界启动检查重跑](https://github.com/Karl-Dai/SimLab-Desktop/actions/runs/39773937303)：提交 `04d26a3`，进行中。增加独立文件描述符、至多 64KiB 的固定日志读取、START 标记、解压与 Xvfb/进程检查超时、Xvfb 错误输出；尚未据此确认旧任务等待的根因。

## 尚未完成

- Windows、Linux 的完整原生 CI 与运行验收；macOS Intel 的窗口和协议操作验收。
- 无 WebView2 的干净 Windows 断网安装；Linux 声明基线上的完整窗口与协议操作。
- macOS 最低版本 13.0 的实机验收；本轮本机系统为 macOS 27.0.1。
- Developer ID 签名、Apple 公证和正式发布。

安装包与 stage 内容的 37 个文件路径、哈希和权限一致。通过本机 CLI `open` 的一次启动出现黑屏及 WebKit/XPC 异常；停止该验证进程后，原生应用接口首次启动正常，主窗口显示 8/8 就绪。具体系统机制未确认，未通过修改资源或关闭保护规避。
