# 周期变位下沉到点位（设计）

日期：2026-06-09

## 背景与问题

子站当前的「固定变位」在 `RemoteParamsForm.vue` 的独立面板里：手填 IOA + ASDU 类型 + 周期，再点启动。存在两个问题：

1. **从未生效**：前端 `asduTypeOptions` 发送的是 `m_me_nc_1`（数字前多一道下划线），而后端 `AsduTypeId` 的 serde 命名是 `m_me_nc1`，`set_fixed_mutation` 对任何类型都会反序列化失败（截图里的 `unknown variant` 报错）。
2. **设计割裂**：要变位的点明明就在点表里看得见，却要去另一个面板重新手敲它的 IOA 和类型。

## 核心思路

把周期变位挂到点位上。点位自带 IOA 和类型，**于是只剩「周期」一个参数要填**，类型/IOA 手填环节连同那个不匹配 bug 一起消失。后端从「单任务」改为「按点位多任务并发」。

## 当前实现（基线）

- `slave.rs::set_fixed_mutation`：维护单个 `fixed_mutation_handle: Mutex<Option<JoinHandle>>`，周期 `flip_value` 一个 `(ioa, type)`，遍历所有 station 翻转匹配点。
- `flip_value`：开关量翻转、StepPosition 轮转、Bitstring/测量值取负、IntegratedTotal +1。
- `RemoteOperationConfig.fixed_mutation: FixedMutationConfig` 随 `remote_ops` 持久化。
- 前端入口：`RemoteParamsForm.vue`（表单）+ `RemoteParamsDrawer.vue`/`RemoteParamsModal.vue`（`actions-fixed` 槽位的启停按钮）+ `useRemoteParams.ts::setFixedMutation`。
- 测试：`headless_mutation_pacing.rs` 含 3 个固定变位测试。

## 目标设计

### 后端（Rust）

**`slave.rs` — `SlaveServer`**

- 删 `fixed_mutation_handle`，换成
  `point_mutation_handles: Mutex<HashMap<(u16, u32, AsduTypeId), JoinHandle<()>>>`，key = `(ca, ioa, asdu_type)`。
- 删 `set_fixed_mutation`，新增：
  - `async fn start_point_mutation(&self, ca: u16, ioa: u32, asdu_type: AsduTypeId, period_ms: u32)`
    同 key 已存在先 abort；spawn 周期任务，**只对该 ca 的该点** `flip_value` + `mark_changed` + `do_queue_spontaneous`（复用现有逻辑，去掉「遍历所有 station」）。`period_ms` 下限 50ms。
  - `async fn stop_point_mutation(&self, ca: u16, ioa: u32, asdu_type: AsduTypeId)`：取出并 abort。
  - `async fn list_point_mutations(&self) -> Vec<(u16, u32, AsduTypeId)>`：返回活跃 key。
- server stop / shutdown 流程 abort 全部句柄并清空 map。

**`slave.rs` — 配置**

- 从 `RemoteOperationConfig` 删 `fixed_mutation` 字段（`#[serde(default)]` 保证旧配置含该字段时被静默忽略），删 `FixedMutationConfig` 结构。`RandomMutationPacing` 保留。
- 周期变位视为运行期仿真行为，**不再持久化**（save/load 不写不读）。

**`commands.rs` + `lib.rs`**

- 删 `set_fixed_mutation` 命令与 `FixedMutationRequest`，删 `FixedMutationConfig` import。
- 新增命令（均含 `server_id` + `common_address` + `ioa` + `asdu_type`）：
  - `start_point_mutation`（额外 `period_ms`）
  - `stop_point_mutation`
  - `list_point_mutations` → `Vec<{ ioa, asdu_type }>`
- `lib.rs` invoke_handler 替换注册（line 50 处）。

### 前端（Vue）

**`DataPointTable.vue`（主战场）**

- 维护 `activeMutations: Set<"ioa:asdu_type">`，随 2s 轮询 + 右键菜单打开时调 `list_point_mutations` 刷新。
- 右键菜单扩展（现有仅「删除」）：
  - 选中点**未变位** → 「启动周期变位」+ 内嵌周期 `<input type="number" min="50" max="60000">`（默认 1000ms，组件内记住上次值；`@click.stop` 防止输入时关菜单；回车或点按钮生效）。
  - 选中点**已变位** → 「停止周期变位」。
  - 多选时对所有选中点应用同一周期。
- 变位中的行加 `mutating` class + 一个小脉冲圆点指示（放 IOA 列）。
- 调 `start_point_mutation` / `stop_point_mutation`，操作后刷新 `activeMutations`。

**清理旧面板**

- `RemoteParamsForm.vue`：删「固定变位」分组（约 217–239 行）、`asduTypeOptions`、`actions-fixed` 槽位；保留「随机变位节流」。
- `RemoteParamsDrawer.vue` / `RemoteParamsModal.vue`：删 `actions-fixed` 模板、`startFixed`/`stopFixed`、dirty 基线里对 `fixed_mutation.enabled` 的特例。
- `useRemoteParams.ts`：删 `setFixedMutation` 及其 export。
- `types.ts`：删 `FixedMutationConfig`、`RemoteOperationConfig.fixed_mutation`、默认值里的 `fixed_mutation`、`asduTypeOptions`；加 `list_point_mutations` 返回类型。

### i18n

- 删 fixed mutation 相关 key（zh-CN + en-US）。
- 加 `table.startMutation` / `table.stopMutation` / `table.mutationPeriod`。

## 保持不变

- `flip_value` 语义（开关量翻转、测量值取负、累计量 +1）。
- `随机变位节流`（`RandomMutationPacing`）与一次性 `random_mutate_data_points`。

## 测试与验证

- **后端**：重写 `headless_mutation_pacing.rs` 的 3 个固定变位测试为新 API
  - 单点 `start_point_mutation` 后轮询确认值翻转并上送 spontaneous；`stop` 后停止。
  - 多点并发：两个点各自独立启停，互不影响。
  - server stop 后句柄全部清理，无任务泄漏。
- **前端**：`npm run build` 通过 + Playwright 真机验证右键启停与指示灯（遵循前端无头浏览器验证规则；jsdom 不算数）。

## 取舍记录

- **多点并发 vs 单点**：选多点并发，更贴合「挂在点位上」的直觉，代价是 `set_fixed_mutation` 重写为多任务管理。
- **周期输入位置**：选右键菜单内嵌输入（复用现有右键删除入口），而非新增表格列（避免表格变重、占横向空间）。
- **周期作用域**：所有选中点共用一次填入的周期（不为每点单独记忆周期），保持菜单简洁。
- **持久化**：不持久化周期变位，定位为运行期仿真。
