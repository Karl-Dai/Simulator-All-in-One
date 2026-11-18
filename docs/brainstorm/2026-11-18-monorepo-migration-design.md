# Monorepo 迁移设计

**Status:** Approved

日期：2026-11-18。套件版本背景：0.2.0 已在 `main` 完成开发并推送，未发布。

## 1. 背景与决策

现有架构：主仓库保存启动器、共享代码（`ui/`、`storage/`）、组装脚本和 6 份补丁；六个组件仓库各自保存协议实现源码。`components.lock.json` 固定每个组件的 commit，`scripts/suite.py bootstrap` 克隆到 `.components/` 并应用补丁后构建。`scripts/sync_ui.py` 保证补丁中的共享资源与主仓库逐字节一致。

这套机制的两个存在理由已经变化：

- 组件仓库全部由同一人维护，"保护上游不被套件修改"是自设约束。
- 用户确认：六个模拟器的独立版本逐步停止单独发布，用户只通过套件获取。

已确认的决策（2026-11-18）：

1. 独立版逐步停发，只保留套件。
2. 迁移一次完成：在分支上合并、验证，通过后切换 `main`。切换前旧机制照常可用。

现有机制的局限（迁移要消除的问题）：

- 补丁共约 21 万行，其中约 12 万行是图标二进制 diff；四个老组件的代码补丁约 8.8 万行，含共享代码完整副本。补丁无法在编辑器中跳转审查。
- 开发在一次性克隆里进行，改动靠 `sync_ui.py --write` 回收进补丁，存在漂移风险（本次检查发现 `iec102` 克隆有图标改动未回收）。
- 共享代码有多份副本，改一次 `storage/` 要重新生成 6 份补丁并全量复验。
- 组件 commit 前移时补丁冲突只能手工解，没有三方合并工具。
- 四个老组件靠补丁适配，`iec101`/`iec102` 的套件支持已在各自上游仓库中，两种集成方式并存。

## 2. 目标与非目标

目标：

- 全部源码进入 `Karl-Dai/Simulator-All-in-One` 一个仓库（monorepo，单仓库模式），保留组件提交历史。
- 共享代码只有一份，组件直接引用，删除副本和同步机制。
- 删除补丁、`bootstrap`、锁文件 commit 固定这套组装流程。
- 四平台构建、打包、发布能力不变，验收标准不降低。

非目标：

- 不合并 Cargo workspace（Rust 的多包工作区），避免重名 crate（Rust 的包）改名，见 §4.3。
- 不做统一宿主单窗口改造。
- 不改协议功能。
- 不重建独立版发布机制（独立版逐步停发）。
- SimLab 网站下载聚合改为指向套件 Release，属于后续项，不在本设计实施范围。

## 3. 目标结构

在现有仓库内合并，不新建仓库：

```
app.go / main.go / internal/ / frontend/   启动器，不变
ui/                                        共享前端源码，唯一一份
storage/go/                                共享 Go 存储包，根模块的一部分
storage/rust/                              升级为 saio-shared crate
components/
  modbus/                                  原 ModbusSim
  iec104/                                  原 IEC60870-5-104-Simulator
  iec101/                                  原 IEC60870-5-101-Simulator
  iec102/                                  原 IEC60870-5-102-Simulator
  pmu/                                     原 PmuSim
  opcua/                                   原 OPCUASim
scripts/                                   suite.py 精简，sync_ui.py 退役
packaging/                                 保留打包逻辑，locks/ 副本退役
.github/workflows/                         重写
```

组件目录内部布局与原仓库一致。组件内部的相对路径（`tauri` 配置、前端引用、crate 间 `path` 依赖）因此保持有效，迁移只处理跨组件引用。

历史保留方式：`git subtree add --prefix=components/<id> <url> <锁文件固定 commit>`。随后用 `git apply --directory=components/<id>` 应用现有补丁并提交，形成"上游历史 + 适配提交"的结构。`iec101`/`iec102` 的补丁只含图标和清单，同样处理。

导入起点必须是"固定 commit + 已提交补丁"，不使用 `.components/` 克隆的当前工作区状态。迁移前先做一次干净 `bootstrap` 并运行 `sync_ui.py --check`，确认补丁与共享资源一致；克隆里的未回收改动（如 `iec102` 图标）先人工核对，属于有效改动的回收进补丁，其余丢弃。

切换完成后六个旧仓库设为只读归档。第一个 monorepo 版本为 0.3.0，组件不再保留独立版本号，`sourceVersion` 字段删除。

## 4. 共享代码接线

### 4.1 前端

各组件前端的 `vite` 配置增加别名，把 `simlab-ui` 指向仓库根 `ui/`。组件源码中的 `./simlab-ui/...` 相对导入改为别名导入。别名目标通过向上查找仓库根标记文件（`components.json`）解析，套件构建和组件本地开发都可用。

删除各前端中的 `simlab-ui/` 副本和 `.source.json` 清单。`scripts/sync_ui.py` 及其构建前检查退役。`ui/` 的改动从此直接生效于所有组件，一致性由单一来源保证。

### 4.2 Go

根目录新增 `go.work`，成员为根模块和 `components/modbus/go`。Modbus 直接导入根模块的两个包：`storage/go`（导入路径 `github.com/Karl-Dai/Simulator-All-in-One/storage/go`）和 `ui` 目录（包名 `suitehost`，对应原 `ui/preferences.go`）。删除 `go/internal/configstore/` 和 `go/internal/suitehost/appearance.go` 副本。

`wails` 构建在根目录和 Modbus 目录执行，`go.work` 向上查找生效。CI 对两个模块分别运行 Go 测试，确认依赖一致。

### 4.3 Rust

`storage/rust/` 升级为真实 crate `saio-shared`：新增 `Cargo.toml` 和 `lib.rs`，包含 `config_store.rs`、`config_close.rs`、`ui/preferences.rs`（改名 `preferences.rs`）。`storage/go/schema.sql` 和 `ui/contracts/appearance-cases.json` 用 `include_str!` 嵌入。

各组件的应用 crate 用 `path` 依赖引用 `saio-shared`，删除 `crates/shared/` 中对应的同步副本（`config_store.rs`、`config_close.rs`、`simlab_preferences.rs`、`config_schema.sql`、`contracts/appearance-cases.json`）。`crates/shared/` 中各组件自己的其他代码保留不动。同步文件清单以 `sync_ui.py` 的 `assets()` 映射为准。

每个组件保留自己的 Cargo workspace 和 `Cargo.lock`，锁文件直接提交在组件目录。不建根 workspace：五个 Rust 仓库存在 `shared` 等重名 crate，合并 workspace 需要全部改名，收益不抵成本。`packaging/locks/` 的锁文件副本和哈希检查退役，锁文件改动走正常 git 审查。

### 4.4 图标

套件图标的唯一来源是主仓库 `assets/roles/<组件id>/`。现在 `sync_ui.py` 把它们复制进各组件（`frontend/public/suite-icon.svg`、`<project>/suite-icons/` 各尺寸，Modbus 另有 `go/internal/suitehost/icons/<id>.png`），补丁里的二进制 diff 就是这些副本。

迁移后改为构建时注入：`suite.py` 在构建每个组件前按同一映射复制图标，组件树中的生成路径加入 git 忽略。另提供独立入口（`suite.py icons`）供组件本地开发使用。补丁中约 12 万行图标二进制 diff 随之消失。

## 5. 构建系统

`components.lock.json` 改为 `components.json`，`schemaVersion` 2：

- 保留：组件 id、目录、构建入口（`project`/`frontend`）、二进制名、`bundleId`。
- 删除：url、commit、patch、`cargoLock`、`cargoLockSha256`、`sourceVersion`。
- `suiteVersion` 移到 `wails.json` 的 `productVersion` 作为唯一来源，脚本从该处读取。

`suite.py` 删除：`bootstrap`、补丁应用、`assert_source`、克隆状态记录（`record_state`）、`verify_shared_ui`。保留：工具链检查、构建环境设置、`suite.sh`/`tauri`/`wails` 构建调用、资源树检查、签名、打包。Rust 组件继续用 `--features simlab-suite` 和 `tauri_suite_config` 覆盖配置，Modbus 继续走 `go/scripts/suite.sh`。

构建回执写入 `dist/receipts/`（`dist/` 已被 git 忽略），记录：monorepo HEAD commit、资源树哈希、工具链哈希、套件版本。发布构建要求 git 工作区干净，脏工作区直接拒绝，替代原来的"未记录改动导致下次构建失败"机制。

开发流程变化：直接在 `components/<id>/` 编辑和提交，不再经过克隆和补丁回收。`.components/` 目录及其忽略规则删除。

## 6. CI 与发布

工作流重写，形状不变、环节减少：

- 源码检查：`go test -race`/`go vet`、前端测试与生产构建、存储 `harness` 测试、打包脚本测试、脚本语法、`actionlint`。不再需要组件克隆和补丁重放检查。
- 原生构建：四目标（macOS arm64/amd64、Windows x64、Linux x64）从 monorepo 直接构建 12 个角色和启动器。
- 包校验（`validate-package.yml`）：来源核对改为"产物 commit 是 tag commit 祖先 + 组件清单 + 资源树哈希"，删除补丁哈希和组件 commit 核对。
- 发布（`release.yml`）：tag 校验、源码检查、四目标构建、六份归档 SHA256、双语说明，流程保留。

不加路径过滤：单一维护者，全量构建就是验证单位，部分构建省下的时间不抵漏验风险。

现有打包测试中针对 `bootstrap`、补丁、锁文件哈希的用例删除，资源树、签名、便携包、安装保护类用例保留并按新结构更新。

## 7. 迁移步骤（在迁移分支上）

1. 干净 `bootstrap` 现有机制，`sync_ui.py --check` 通过；核对克隆未回收改动，确认导入起点等于已验证的 0.2.0 源码。
2. 建分支 `monorepo-migration`，`subtree` 并入六个仓库的固定 commit，应用补丁并提交。
3. 接共享代码：`vite` 别名与导入改写、`go.work`、`saio-shared` crate；删除各处副本。
4. 精简 `suite.py`，改 `components.json`，退役 `sync_ui.py`、`patches/`、`packaging/locks/`。
5. 重写工作流；更新打包测试。
6. 本地验证（§8）通过后推送分支，触发四目标原生构建。
7. 切换：合并进 `main`，更新 README 和文档，归档六个旧仓库（GitHub 操作，需用户执行或授权）。

切换前 `main` 不动。切换后回滚方式：`revert` 合并提交，恢复旧仓库归档状态，旧机制在历史中完整可用。

## 8. 验证计划

切换门槛，全部通过才合并 `main`：

| 层 | 内容 |
| --- | --- |
| 单元/集成 | `go test -race`/`go vet`（启动器、存储、Modbus）、前端 `vitest`、存储 `harness` 双语言互读、打包脚本测试 |
| 构建 | 四目标原生构建 12 角色 + 启动器，资源树与签名检查，便携 EXE 与 DMG/NSIS/`.run` 打包 |
| 界面 | 启动器无头浏览器检查（双语、主题、12 入口、收藏）；macOS ARM 真实窗口 12/12 就绪 |
| 协议冒烟 | Modbus FC03/FC06/FC16 读写；IEC104 STARTDT 与总召；IEC101/102 链路建立与召唤；PMU 握手与帧接收；OPC UA 连接与变量读写 |
| 存储 | 旧 `saio.db` 和旧自动保存文件在迁移后版本中正常迁移读取 |

历史验收证据只证明旧机制；四平台完整窗口与协议验收缺口（Windows/Linux/macOS Intel 实机、断网首装、macOS 13、签名公证）延续原有清单，不因迁移关闭。

## 9. 风险

- **跨组件引用遗漏**：组件内部相对路径不变，但逃逸到原仓库根的引用（CI 脚本、文档链接、构建脚本中的根路径）需要逐一排查。缓解：全量构建 + 全文搜索旧仓库名。
- **依赖解析变化**：`saio-shared` 引入新的 `path` 依赖边，各组件 `Cargo.lock` 会变化。缓解：锁文件改动逐个审查，构建用 `--locked`。
- **验证周期长**：四目标原生构建加协议冒烟需要完整一轮。缓解：迁移期间不发版，`main` 保持可构建。
- **旧仓库归档后的引用失效**：README、文档、SimLab 网站中的旧仓库链接。缓解：切换时统一更新文档；网站列为后续项。

## 10. 后续项（不在本次实施）

- SimLab 网站下载聚合改为套件 Release。
- 独立版自动更新功能关闭或移除（随组件适配提交处理）。
- 统一 Cargo workspace 的可行性，待组件稳定后另行评估。

## 11. 实施记录（2026-11-18，迁移分支）

与设计的偏差和补充决定：

- 锁文件固定的 iec101 commit `1b1b1123` 与 iec102 commit `a0dacadc` 起初只存在于 `~/Developer/` 的本地开发仓库，各有 3 个提交未推送，已推送的套件 main 在 CI 上无法构建。经用户确认，两个组件仓库的 6 个提交已补推到 GitHub（`bdfd18a..1b1b112`、`cd4a6a3..a0dacadc`）。
- `saio-shared` 的外观模块保留文件名 `simlab_preferences.rs`（设计稿写的是 `preferences.rs`），调用方模块路径不变。`pub(crate)` 改为 `pub`；文件内的 `crate::config_store` 引用和 `simlab-suite` feature 在 crate 内部解析，feature 由各应用的 `simlab-suite = ["saio-shared/simlab-suite"]` 传播。
- `saio-shared` 依赖补齐 `sha2`、`log`、`tempfile`（原先由应用 crate 提供）。
- 清单 `schemaVersion` 保持 1：启动器只读 `sourceCommit`、`version`、`sha256`，不读 `sourceVersion` 和补丁哈希，删除这两个字段后启动器无需改动。`sourceCommit` 记录 monorepo HEAD。
- Windows 短路径方案改为 `CARGO_TARGET_DIR=D:/t`，`suite.py` 的 `payload_executable` 经 `cargo_target()` 读取产物；Windows 的 rust-cache 不缓存 target 目录。
- `.components/` 忽略规则保留：旧克隆仍在磁盘上，其中 iec102 的未回收图标经逐字节比对与 `assets/roles/` 完全一致，属同步副本，可安全删除整个目录。
- pmu 两个前端的 `tsconfig.node.json` 是 composite 项目（vue-tsc 2.x），需要 `rootDir` 指到仓库根并把输出重定向到 `node_modules/.tmp`；modbus/iec104/opcua 的 node 项目用 `noEmit`，无此问题。
- iec101/102/104 上游 `.gitignore` 原本忽略 `Cargo.lock`，已移除该行，锁文件改为提交进组件目录。