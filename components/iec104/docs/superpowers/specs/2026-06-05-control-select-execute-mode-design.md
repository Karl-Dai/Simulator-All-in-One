# 遥控命令支持"仅选择 / 仅执行 / 自动两步"模式

- 日期:2026-06-05
- 状态:已批准设计,待实现
- 范围:IEC104 主站模拟器(master-frontend + iec104master-app + iec104sim-core)

## 背景与问题

遥控/控制命令(C_SC_NA_1 等)的 SCO/DCO/RCO/QOS 限定词里有一个 S/E 位(bit7,Select=1 / Execute=0)。当前主站的「自定义控制」对话框只暴露一个 `选择-执行(SbO)` 复选框:

- 不勾选 → `select=false` → 直接发一条 S/E=0 的执行帧,发完即返回。
- 勾选 → `select=true` → 后端自动两步(发选择帧 → 等 ACT_CON → 发执行帧 → 等 ACT_CON)。

用户无法**单独下发一条选择帧(S/E=1)**用于调试(例如单独观察子站对 Select 的 ACT_CON 响应、测试子站 SBO 超时行为、在两步之间插入观察)。参照对端工具的做法,需要一个明确的 S/E 模式选择。

关键事实:用户口中的"只下发执行",现在不勾 SbO 的"直接执行"就是单发一条 S/E=0 帧,已能做到。真正缺的是"只下发选择"。

后端核心库 `send_*_command(..., select: bool, ...)` 本就接受 S/E 布尔,直接执行分支只是写死传 `false`。因此"仅选择"在后端几乎零新增——把写死的 `false` 换成由模式决定的布尔即可。

## 目标 / 非目标

目标:
- 遥控对话框用"控制模式"三选一(仅执行 / 仅选择 / 自动两步)取代现有 SbO 复选框。
- 适用所有带 S/E 位的控制类型:单点 C_SC、双点 C_DC、步调 C_RC、三种设点 C_SE_NA/NB/NC。
- 位串 C_BO_NA_1 协议上无 S/E 位,保持只能"仅执行"。
- 向后兼容现有 `select` 参数语义。

非目标(YAGNI):
- 不做"任意报文类型 / COT / SCS 全字段手填"的自由报文面板(已有独立的报文解析 / RawSend 入口)。
- 不改 SBO 超时时长、不动总召唤/累计量等其他命令。

## 行为定义

| 模式 | 报文 | S/E 位 | 确认等待 | 返回 step.action |
|---|---|---|---|---|
| 仅执行 Execute(默认) | 单发 1 条 | 0 | 不等待,发完即返回 | `execute_sent` |
| 仅选择 Select(新增) | 单发 1 条 | 1 | 不等待,发完即返回 | `select_sent` |
| 自动两步 SBO | 选择帧→等 ACT_CON→执行帧→等 ACT_CON | 1 then 0 | 两次,各 5s 超时(现状) | 4 步 |

决策(已拍板):"仅选择/仅执行"**不阻塞等待 ACT_CON**。子站回的 ACT_CON 会照常进通信日志异步显示,调试时一样可见。这与现有"直接执行"行为一致。

## 后端设计(iec104master-app/src/commands.rs,复用 iec104sim-core)

1. `ControlCommandRequest` 新增字段 `control_mode: Option<String>`,取值 `"execute"` / `"select"` / `"sbo"`。
2. 保留现有 `select: Option<bool>` 字段做兼容。模式解析优先级:
   - 若 `control_mode` 存在,直接用它。
   - 否则回退旧语义:`select == Some(true)` → `sbo`;其余 → `execute`。
3. `send_control_command` 分支重构:
   - `sbo` → 走现有 SBO 分支(构造 select 帧 + execute 帧,委托 `send_control_with_sbo_event`),不变。
   - `execute` / `select` → 走现有"直接发一条"分支,唯一改动:把写死的 `false` 换成 `sel = (mode == "select")`,透传给 `send_single_command / send_double_command / send_step_command / send_setpoint_normalized / send_setpoint_scaled / send_setpoint_float`。
   - 返回的 `ControlStep.action` 按模式记 `select_sent` 或 `execute_sent`。
4. 位串 C_BO_NA_1(`send_bitstring_command` 无 select 参数,SCO 无 S/E 位):
   - `select` / `sbo` 模式 → 返回明确错误(同现状 SBO 报错文案思路)。
   - 仅 `execute` 允许。
5. 不新增任何帧构造函数;`build_control_frames_*` 与核心库 `build_*_command` 全部复用。

## 前端设计(master-frontend/src/components/ControlDialog.vue)

1. 状态:`selectMode: boolean` → `controlMode: 'execute' | 'select' | 'sbo'`,默认 `'execute'`(等于旧默认 `selectMode=false`)。
2. 持久化(localStorage `iec104master.controlDialog.v1`)迁移:
   - 读到旧 `selectMode === true` → `controlMode = 'sbo'`;否则 `'execute'`。
   - 写入新增 `controlMode` 字段。
3. UI:原 `toggle-row` 复选框替换为「控制模式」下拉 `<select>`(与命令类型下拉同风格),三个选项走 i18n。下拉旁保留一句 hint 说明当前模式语义。
4. 位串类型时:下拉禁用并锁定为"仅执行",hint 显示"位串无 S/E 位"。
5. payload:发送 `control_mode: controlMode.value`;同时保留 `select`(由模式映射:`controlMode === 'sbo'`)做兼容。
6. 结果展示:三模式统一走现有 `lastResult.steps` 点阵 + duration;"仅选择"显示一个 `select_sent` 点。

## i18n(master-frontend/src/i18n/locales/zh-CN.ts、en-US.ts、DictShape 类型)

`control` 段新增 key:
- `controlMode`(标签:控制模式 / Control Mode)
- `modeExecute`(仅执行(Execute) / Execute only)
- `modeSelect`(仅选择(Select) / Select only)
- `modeSbo`(自动两步(SBO) / Auto two-step (SBO))
- 三个模式各一句 hint(可复用现有 `sboTwoStep` 作为 SBO 的 hint)。

`DictShape` 类型同步新增上述 key(否则 `vue-tsc -b` 报错)。保留旧 `sboLabel/sboTwoStep/sboDirect/bitstringNoSbo`。

## 测试策略

1. Rust 集成测试 `crates/iec104sim-core/tests/control_e2e.rs` 新增"仅选择单步"用例:
   - 以 `control_mode = "select"` 发单点命令,断言:只发 1 条帧、SCO bit7 = 1(S/E=Select)、返回 step.action = `select_sent`、不发第二条执行帧。
2. 前端 Playwright(沿用记录型 Tauri mock,headless):
   - 选"仅选择" → `send_control_command` 的 payload `control_mode === 'select'` 且仅调用一次。
   - 选"自动两步" → payload `control_mode === 'sbo'`,后端走 SBO 路径。
   - 位串类型 → 模式下拉锁定"仅执行"。
3. 构建门禁:`npm run build`(vue-tsc + vite)通过;`cargo test -p iec104sim-core` 通过。

## 关键代码位置参考

| 功能 | 文件 | 行号(批准时) |
|---|---|---|
| 后端主处理 `send_control_command` | iec104master-app/src/commands.rs | 495–665 |
| 直接执行分支(写死 false) | iec104master-app/src/commands.rs | 518–568 |
| SBO 分支 | iec104master-app/src/commands.rs | 571–664 |
| `ControlCommandRequest` 结构 | iec104master-app/src/commands.rs | ~460–471 |
| 帧构造 build_control_frames_* | iec104master-app/src/commands.rs | 759–809 |
| 核心库单发函数(select 参数已具备) | iec104sim-core/src/master.rs | 1069–1143 |
| SBO 两步流程 | iec104sim-core/src/master.rs | 1156–1255 |
| 命令类型常量 | iec104sim-core/src/types.rs | 6–116 |
| 前端对话框 selectMode/payload/UI | master-frontend/src/components/ControlDialog.vue | 52 / 162–175 / 328–334 |
| SBO 集成测试 | iec104sim-core/tests/control_e2e.rs | 188–257 |

## 验收标准

- 遥控对话框出现"控制模式"三选一,默认"仅执行",行为与旧"直接执行"一致。
- 选"仅选择"对任一带 S/E 的类型发送时,抓包/通信日志显示一条 S/E=1 的帧,且不自动跟发执行帧。
- 选"自动两步"行为与现状 SBO 完全一致。
- 位串类型无法选择 select/sbo。
- 旧持久化的 `selectMode=true` 升级后映射为"自动两步"。
- Rust + 前端构建与测试全绿。
