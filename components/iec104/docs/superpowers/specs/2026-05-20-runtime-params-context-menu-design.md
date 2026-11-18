# 子站/服务器节点右键修改运行参数 — 设计

日期：2026-05-20
分支：feat/point-config-import-export

## Context

子站界面的 `ConnectionTree` 当前右键菜单只支持：
- server 节点：启动 / 停止 / 删除
- station 节点：删除

运行参数（协议时序 t0/t1/t2/t3/k/w + 远动运行参数 + 固定变位）只能从常驻侧边面板 `RemoteParamsPanel.vue` 编辑，并且要求先在树上选中目标 server。

用户反馈"当前子站右键不能修改参数"。期望：在 server 节点和 station 节点的右键菜单里都加一个「修改运行参数」入口，点击后弹出独立对话框集中编辑该 server 的全部运行参数。

后端运行参数为**服务器级**存储（所有子站共享一套）。本次不下沉到 per-station，以最小改动交付。station 节点右键打开的对话框等价于编辑其所属 server 的全局参数（与现有侧边面板语义一致）。

## 范围

- 仅前端改动，零后端改动
- 复用现有 Tauri 命令：`get_protocol_timing` / `set_protocol_timing` / `get_remote_operation_config` / `set_remote_operation_config` / `set_fixed_mutation`
- 侧边面板 `RemoteParamsPanel.vue` 保留，与对话框并存

## 设计

### 组件改动

1. **抽出表单组件 `frontend/src/components/RemoteParamsForm.vue`（新文件）**
   - 把 `RemoteParamsPanel.vue` 模板中的字段控件（三个 section：协议时序 / 远动运行参数 / 固定变位）整体迁移到本组件
   - props：
     - `timing: ProtocolTimingConfig`（v-model:timing）
     - `ops: RemoteOperationConfig`（v-model:ops）
   - 不含「应用」按钮、不含 fixed_mutation 启停按钮 —— 这些保留在容器内（Panel 保留分段「应用」，Modal 统一「保存」）
   - 仅承载字段渲染 + 双向绑定，无 Tauri 调用

2. **重构 `RemoteParamsPanel.vue`**
   - 模板中的字段块替换为 `<RemoteParamsForm v-model:timing="timing" v-model:ops="ops" />`
   - 保留：折叠/展开壳、各 section 的「应用」按钮、固定变位开始/停止按钮、`useRemoteParams` 调用、错误/loading 提示
   - 行为与现状一致

3. **新增 `frontend/src/components/RemoteParamsModal.vue`**
   - props：`serverId: string`、`open: boolean`
   - emits：`close`
   - 内部维护 `serverIdRef = ref(props.serverId)`，传入 `useRemoteParams(serverIdRef)` 拿到独立的 timing/ops snapshot（不污染全局 `selectedServerId`）
   - 打开时 useRemoteParams 的 `watch(selectedServerId, load, { immediate: true })` 自动加载
   - 模板：标题（显示 server name / id）+ `<RemoteParamsForm>` + 底部按钮区「保存」「取消」
   - 「保存」：顺序调用 `applyTiming()` → `applyOps()`；若改动了 `fixed_mutation`，再调 `setFixedMutation(ops.fixed_mutation)`；任一步失败显示 `lastError` 并保持对话框打开
   - 「取消」/ ESC / 遮罩点击：触发 `close`
   - 视觉上对齐项目其他 Modal（参考 `DataPointModal.vue` / `BatchAddModal.vue`）

4. **修改 `ConnectionTree.vue` 右键菜单（约 280-295 行）**
   - server 节点：在「启动/停止」与「删除」之间插入新项「修改运行参数」`tree.ctxEditRuntimeParams`
   - station 节点：在「删除」之前插入新项「修改运行参数」（同 i18n 键复用）
   - 处理函数 `ctxEditRuntimeParams`：
     - server 节点直接拿 `contextMenu.serverId`
     - station 节点拿 `contextMenu.serverId`（station 上下文已携带其所属 server id；若未携带需在 menu open 处补传）
     - 通过 emit 或全局事件通知父级（`App.vue`）打开 Modal 并传入目标 serverId

5. **`App.vue`**
   - 接收 ConnectionTree 的「打开运行参数对话框」事件 → 控制 `RemoteParamsModal` 的 `open` 与 `serverId`

### 字段范围（Modal 与 Panel 完全一致）

- 协议时序：t0, t1, t2, t3, k, w（来自 `ProtocolTimingConfig`）
- 远动运行：sp_sync_with_tb, answer_general_interrogation, answer_counter_interrogation, answer_commands, gi_include_timestamped, upload_mode_untimestamped, upload_mode_timestamped, select_ack_cot, execute_ack_cot, cancel_ack_cot, random_pacing.{batch_size, delay_ms}, auto_packing
- 固定变位：fixed_mutation.{ioa, asdu_type, period_ms, enabled}

### i18n

新键（中 / 英）：
- `tree.ctxEditRuntimeParams` — 「修改运行参数」/ "Edit Runtime Params"
- Modal 相关：`runtimeParams.title`, `runtimeParams.save`, `runtimeParams.cancel`

定位现有 locale 文件后追加。

### 测试 / 验证

- 单元测试：暂无（Vue 组件本仓未配置组件测试）
- 手动验证：
  1. `pnpm tauri dev` 启动
  2. 新建一个 server，添加一个 station
  3. 在 server 节点右键 → 点「修改运行参数」→ Modal 弹出，字段加载正确 → 改 t0 / GI 应答开关 / 固定变位 IOA → 保存 → 关闭 → 重新打开核对持久化
  4. 在 station 节点右键 → 点「修改运行参数」→ Modal 弹出且数据等于其所属 server → 修改保存 → 通过侧边面板（选中该 server）确认值已同步
  5. 错误路径：关闭 server 后右键修改 → 期望 `lastError` 提示，不崩溃

## 不做的事

- 不在本次下沉为 per-station 参数（业务上 t0/t1/t2/t3/k/w 是连接级，不应按子站分；其他字段下沉留待后续业务诉求明确后再做）
- 不删除侧边面板（保留两条路径，待后续 UX 评估）
- 不增加新的 Tauri 命令
- 不修改 Rust 端结构体

## 涉及文件

- 新增：
  - `frontend/src/components/RemoteParamsForm.vue`
  - `frontend/src/components/RemoteParamsModal.vue`
- 修改：
  - `frontend/src/components/RemoteParamsPanel.vue`（提取字段块）
  - `frontend/src/components/ConnectionTree.vue`（菜单项 + 事件 emit）
  - `frontend/src/App.vue`（Modal 挂载 + 事件接线）
  - `frontend/src/locales/zh.ts` 与 `en.ts`（新增 i18n 键，路径待确认）
