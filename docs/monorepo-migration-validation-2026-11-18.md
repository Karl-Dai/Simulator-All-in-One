# Monorepo 迁移本地验证记录

日期：2026-11-18。分支 `monorepo-migration`，设计文档见 [monorepo 迁移设计](brainstorm/2026-11-18-monorepo-migration-design.md)。本机 macOS Apple Silicon，工具链按 `toolchain.lock.json` 固定版本（node 22.23.4、npm 10.9.9、Python 3.12.10、go 1.27.3、rust 1.99.1、wails 2.14.0）。

## 导入一致性

- 六个组件仓库以 subtree 方式并入 `components/<id>/`，保留完整提交历史。
- 导入树与锁文件 commit 逐文件比对（路径、模式、blob 哈希）：modbus 497、iec104 356、pmu 189、opcua 218、iec101 128、iec102 136 个文件全部一致。
- 应用已提交补丁后，与干净基线克隆（固定 commit + 补丁 + `sync_ui.py --check` 通过）逐字节 diff：六个组件全部一致。
- 旧 `.components/` 克隆中 iec102 的未回收图标与 `assets/roles/` 逐字节一致，属同步副本，无有效改动丢失。

## 自动化测试

| 范围 | 结果 |
| --- | --- |
| 根模块 `go test -race`（启动器、`internal/suite`、`internal/webviewruntime`、`storage/go`、`ui`）与 `go vet` | 通过 |
| Modbus `go test -tags simlab_suite ./...`（含 cmd 与 internal） | 通过 |
| 启动器前端 vitest 65 项、生产构建 | 通过 |
| 12 个组件前端生产构建（`vue-tsc` + vite，套件模式） | 全部通过 |
| `saio-shared` crate 测试（默认与 simlab-suite feature） | 9 项通过 |
| 存储 harness（Rust 迁移、陈旧写入、类型值、工程导出） | 4 项通过 |
| 协议核心 crate：iec101 51、iec102 44、iec104 267、OPC UA 103、PMU 66 | 全部通过 |
| 打包脚本测试（重写后） | 69 项通过，1 项按平台跳过 |

## 原生构建与打包（darwin/arm64）

- `suite.py build`：12 个角色应用 + 启动器构建成功，回执写入 `dist/receipts/`，记录 monorepo HEAD。
- `suite.py collect`：嵌套 ad-hoc 签名，资源树哈希核对。
- `suite.py verify`：12/12 角色通过。
- `suite.py package`：DMG 与 ZIP 生成，`hdiutil verify` 校验和有效。产物 `dist/packages/SAIO_0.2.0_darwin-arm64.{dmg,zip}`。

## 协议与启动冒烟

- IEC101 命令行主从对（`iec101-pair`）真实 TCP：链路复位与认可帧交换正常，总召唤返回监视点（布尔、双精度、浮点），遥控命令执行完成，主站进程正常退出。
- IEC102 命令行主从对（`iec102-pair`）真实 TCP：累计量召唤返回三个计量点（100/200/300），进程正常退出。
- 打包产物中 12 个 Helper 应用与启动器逐个以隔离 `SIMLAB_DATA_DIR` 启动，3 秒后进程全部存活，随后正常关闭。

## 未覆盖项

- Windows、Linux、macOS Intel 原生构建与验收：待分支推送后由 CI 执行。
- IEC104、PMU、OPC UA、Modbus 的 GUI 协议互联：本轮以核心 crate 测试和 Modbus Go 测试覆盖协议逻辑，未做窗口级联调；与历史验收边界一致。
- 启动器无头浏览器交互检查：本轮以前端 vitest 与生产构建覆盖，未重做 Playwright 视口检查。
- 旧 `saio.db` 实机迁移：存储迁移逻辑由 `storage/go` 与 harness 测试覆盖，未在真实用户数据目录执行。
- 历史遗留缺口不变：无 WebView2 的干净 Windows 断网首装、macOS 13 实机、Developer ID 签名与公证。

## 过程中的修复

- 锁文件固定的 iec101/102 commit 起初未推送到 GitHub，套件 main 在 CI 上不可构建；经用户确认补推 6 个提交。
- pmu 两个前端的 composite tsconfig 需要 `rootDir` 指向仓库根并重定向输出，否则根级 vite 辅助函数触发 TS6059/TS6307。
- Modbus 上游跟踪的 wails 绑定是独立版生成结果；套件构建重新生成后提交，避免每次构建弄脏工作区。
- 构建把根 go.mod 的 go 指令升到 1.27 并调整 sqlite 依赖标记，已提交。