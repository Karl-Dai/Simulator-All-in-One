# 主站「发送应用层报文」对话框 + 取消自动 GI

日期：2026-06-02
范围：`crates/iec104sim-core`、`crates/iec104master-app`、`master-frontend`、`shared-frontend/i18n`

## 目标

复刻经典 IEC-104 主站调试工具的「发送应用层报文」对话框：用户可手选 TypeID / COT / 公共地址 / 信息体地址 / 各类品质字段，发出一条任意 ASDU。同时，**取消默认的「连接后自动总召唤」行为**，改为按连接级别的开关控制，默认关闭。

入口位置：替换工具栏现有「自定义控制」按钮（改名「发送报文」），新建独立对话框 `SendAsduDialog`。DataTable 右键单点的「设值/控制」继续走旧的 `ControlDialog`（语义更友好，不改）。

## 不做的事

- 不删 `ControlDialog.vue` —— 右键单点设值仍然需要它。
- 不支持时标命令（C_SC_TA_1=58 / C_DC_TA_1=59 / C_RC_TA_1=60 / C_SE_TA/TB/TC=61/62/63 / C_BO_TA_1=64）：截图原版无，且本期不展开。
- 不做"按 TypeID 把响应帧解析回填到对话框"（双向回填属于"帧解析"工具的范畴）。
- 不做参数下装/参数激活的可视化「我已经下装哪些参数」状态追踪 —— 仅支持发出。
- 不为 P_AC_NA_1 / C_RP_NA_1 等命令提供"预设组合"按钮（QPA/QRP 留给用户填）。

## 架构总览

| 层 | 改动 |
|---|---|
| `core/types.rs` | `AsduTypeId` 新增 7 个枚举项（102/104/105/106/110/111/112/113）；`from_u8`/`name`/`description`/`category=System` 同步 |
| `core/master.rs` | 新增 8 个 builder（read/test/reset/delay/p_me_×3/p_ac）；GI/CI/Clock builder 加 `cot: u8` 参数（保持调用方默认）；`MasterConnection::send_raw_asdu(frame: Vec<u8>)` 公共方法 |
| `core/master.rs` | `MasterConfig` 新增 `auto_gi_on_connect: bool`，`#[serde(default)]` |
| `master-app/commands.rs` | 新增 `send_asdu` 命令；`CreateConnectionRequest` + `ConnectionInfo` 加 `auto_gi_on_connect` |
| `master-frontend` | 新 `SendAsduDialog.vue`；`Toolbar.vue` 改 "自定义控制" 为 "发送报文"，`connectMaster` 改读开关；`NewConnectionModal.vue` 加复选；`types.ts` 加字段 |

## TypeID 与字段映射

### 17 种 TypeID
| TypeID | 编号 | 启用字段 | 备注 |
|---|---|---|---|
| C_IC_NA_1 | 100 | QOI | 默认 20 |
| C_CI_NA_1 | 101 | QCC | 默认 5 (total + no freeze) |
| C_RD_NA_1 | 102 | IOA | 读 |
| C_CS_NA_1 | 103 | IOA、CP56Time2a | IOA=0 |
| C_TS_NA_1 | 104 | — | FBP 固定 0xAA55 |
| C_RP_NA_1 | 105 | IOA、QRP | IOA=0 |
| C_CD_NA_1 | 106 | IOA、delay_ms (CP16Time2a) | IOA=0 |
| C_SC_NA_1 | 45 | IOA、SCS、S/E、QU | |
| C_DC_NA_1 | 46 | IOA、DCS、S/E、QU | DCS 0..=3 |
| C_RC_NA_1 | 47 | IOA、RCS、S/E、QU | RCS 0..=3 |
| C_SE_NA_1 | 48 | IOA、NVA、S/E、QL | NVA: -1.0..=1.0 |
| C_SE_NB_1 | 49 | IOA、SVA、S/E、QL | SVA: i16 |
| C_SE_NC_1 | 50 | IOA、R32、S/E、QL | R32: f32 |
| P_ME_NA_1 | 110 | IOA、NVA、QPM | |
| P_ME_NB_1 | 111 | IOA、SVA、QPM | |
| P_ME_NC_1 | 112 | IOA、R32、QPM | |
| P_AC_NA_1 | 113 | IOA、QPA | |

### 9 种 COT
| COT | 数值 | 标识 |
|---|---|---|
| activation | 6 | act |
| activation con | 7 | actcon |
| deactivation | 8 | deact |
| deactivation con | 9 | deactcon |
| activation term | 10 | actterm |
| return remote | 11 | retrem |
| return local | 12 | retloc |
| file transfer | 13 | file |
| interrogated | 20 | inrogen（对应"响应总召唤"） |

COT 数字框允许 0..=63 任意填，下拉只是预设。TEST 位 / P/N 位作为独立 checkbox。

## `send_asdu` 命令 schema

```rust
// crates/iec104master-app/src/commands.rs
#[derive(serde::Deserialize)]
pub struct SendAsduRequest {
    pub type_id: u8,
    pub cot: u8,
    pub cot_test: Option<bool>,   // TEST 位（默认 false=非测试），bit 7 of COT byte
    pub cot_pn: Option<bool>,     // P/N 位（默认 false=肯定），bit 6 of COT byte
    pub common_address: u16,
    pub ioa: Option<u32>,

    // 品质字段
    pub qoi: Option<u8>, pub qcc: Option<u8>, pub qrp: Option<u8>,
    pub qpa: Option<u8>, pub qpm: Option<u8>, pub qu: Option<u8>,

    // 命令位 / 选择执行
    pub select: Option<bool>,
    pub scs: Option<bool>, pub dcs: Option<u8>, pub rcs: Option<u8>,

    // 数值
    pub nva: Option<f32>, pub sva: Option<i16>, pub r32: Option<f32>,

    // C_CD_NA_1
    pub delay_ms: Option<u16>,

    // C_CS_NA_1
    pub time: Option<Cp56Time2aFields>,
}

#[derive(serde::Deserialize)]
pub struct Cp56Time2aFields {
    pub year: u16, pub month: u8, pub day: u8,
    pub hour: u8, pub minute: u8, pub second: u8,
    pub millis: u16,
    pub dow: u8, pub dst: bool, pub iv: bool,
}

#[tauri::command]
pub async fn send_asdu(
    state: State<'_, AppState>,
    id: String,
    request: SendAsduRequest,
) -> Result<(), String>;
```

- 后端按 `type_id` switch 选 builder，缺字段直接 `Err("missing field xx for type_id yy")`，前端弹错。
- 复用 `MasterConnection::send_raw_asdu` 走 `send_lock` + `ack_notify`，跟既有 send_control_command 共用并发保证。
- TEST + P/N 位通过 `cot |= (test as u8) << 7 | (pn as u8) << 6` 拼到 COT 字节里（与 IEC-101 一致，bit7=TEST, bit6=P/N）。

## Core builder 新增清单

```rust
// crates/iec104sim-core/src/master.rs

// 现有签名 → 新签名（加 cot，调用方传 6）
fn build_gi_command(ca: u16, qoi: u8) -> Vec<u8>
    → fn build_gi_command(ca: u16, qoi: u8, cot: u8) -> Vec<u8>
fn build_counter_read_command(ca: u16, qcc: u8) -> Vec<u8>
    → fn build_counter_read_command(ca: u16, qcc: u8, cot: u8) -> Vec<u8>
fn build_clock_sync_command(ca: u16) -> Vec<u8>
    → fn build_clock_sync_command(ca: u16, time: Cp56Time2a, cot: u8) -> Vec<u8>

// 新建
fn build_read_command(ca: u16, ioa: u32, cot: u8) -> Vec<u8>
fn build_test_command(ca: u16, cot: u8) -> Vec<u8>            // FBP=0xAA55
fn build_reset_process_command(ca: u16, ioa: u32, qrp: u8, cot: u8) -> Vec<u8>
fn build_delay_acq_command(ca: u16, ioa: u32, delay_ms: u16, cot: u8) -> Vec<u8>
fn build_parameter_normalized(ca: u16, ioa: u32, nva: f32, qpm: u8, cot: u8) -> Vec<u8>
fn build_parameter_scaled(ca: u16, ioa: u32, sva: i16, qpm: u8, cot: u8) -> Vec<u8>
fn build_parameter_float(ca: u16, ioa: u32, r32: f32, qpm: u8, cot: u8) -> Vec<u8>
fn build_parameter_activation(ca: u16, ioa: u32, qpa: u8, cot: u8) -> Vec<u8>

// 公共发送
impl MasterConnection {
    pub async fn send_raw_asdu(&self, frame: Vec<u8>, label: FrameLabel) -> Result<(), MasterError>;
}
```

C_CS_NA_1 现签名只取 CA，内部用系统时间；扩展后接受调用方传入 CP56Time2a，旧 `send_clock_sync(ca)` 仍可用 `chrono::Local::now()` 默认填充。

旧的 `spawn_periodic_poller`（master.rs:723）继续传 `cot=6` 调用 GI/CI builder，无行为变化。

## auto-GI 开关

```rust
// MasterConfig
#[serde(default)]
pub auto_gi_on_connect: bool,
```

- 默认 `false`（贴近截图原版）。
- `CreateConnectionRequest`/`ConnectionInfo` 同步新增。
- 老配置（无字段）通过 `serde(default)` 退化为 false。

前端：

```ts
// Toolbar.vue
async function connectMaster() {
  await invoke('connect_master', { id })
  refreshTree()
  const conn = (await invoke<ConnectionInfo[]>('list_connections'))
    .find(c => c.id === id)
  if (conn?.auto_gi_on_connect) {
    try { await fanOutCAs('send_interrogation'); refreshData(); setTimeout(refreshTree, 3000) }
    catch (e) { console.warn('Auto GI after connect failed:', e) }
  }
}
```

`NewConnectionModal.vue` 在 TLS 区块后追加一个 `form-row`：

```vue
<label class="form-checkbox">
  <input type="checkbox" v-model="autoGiOnConnect" />
  {{ t('newConn.autoGi') }}
</label>
```

提交时塞到 `create_connection` payload。

## 前端组件 `SendAsduDialog.vue`

- Teleport 到 body，半透明遮罩，跟 ControlDialog 共用 modal 样式。
- 顶部双列：报文类型（数字 input + 下拉）、传输原因（数字 input + 下拉）、TEST、P/N。
- 中部：公共地址 + 信息体地址；CA 用 ControlDialog 同款 `availableCAs` 下拉 + 自定义模式。
- 字段区按 `typeId` 动态显示/禁用：
  - 用 `enabledFields(typeId)` 返回 `{qoi:true, ioa:true, ...}`，CSS `:disabled` 灰掉。
- 持久化 key：`iec104master.sendAsduDialog.v1`，仅持久化字段值与最后一次选择的 typeId/cot；连接相关字段 (CA) 每次打开重新解析。
- Enter 发送 / Esc 关闭、显示发送中、显示 `lastResult` OK 指示，跟 ControlDialog 风格一致。

## i18n key 新增

```
sendAsdu.title = 发送应用层报文 / Send Application ASDU
sendAsdu.typeId = 报文类型 / Type ID
sendAsdu.cot = 传输原因 / Cause of Transmission
sendAsdu.test = TEST
sendAsdu.pn = P/N
sendAsdu.ca = 公共地址 / Common Address
sendAsdu.ioa = 信息体地址 / Info Object Address
sendAsdu.qoi = QOI
sendAsdu.qcc = QCC
sendAsdu.qu = QU
sendAsdu.qrp = QRP
sendAsdu.qpa = QPA
sendAsdu.qpm = QPM
sendAsdu.scs = SCS
sendAsdu.dcs = DCS
sendAsdu.rcs = RCS
sendAsdu.nva = SAV (归一化值)
sendAsdu.sva = SAV (标度化值)
sendAsdu.r32 = 短浮点数
sendAsdu.delay = 传输延时 (ms)
sendAsdu.timeYear/Month/Day/Hour/Min/Sec/Ms/Dow/Dst/Iv = …
sendAsdu.send = 发送
sendAsdu.sending = 发送中…
sendAsdu.cancel = 取消
toolbar.sendAsdu = 发送报文 / Send ASDU   (替代旧 toolbar.customControl)
newConn.autoGi = 连接后自动总召唤
```

旧 `toolbar.customControl` key 同时保留，不立即删（避免老语言文件被静默漏掉）。

## TypeScript 类型

```ts
// master-frontend/src/types.ts 追加
export interface AsduTypeOption { id: number; name: string; desc: string }
export interface CotOption     { value: number; tag: string; label: string }

// ConnectionInfo
export interface ConnectionInfo {
  ...
  auto_gi_on_connect: boolean
}

// SendAsduRequest（与 Rust 结构对齐，snake_case 由 invoke 映射）
export interface SendAsduRequest {
  type_id: number
  cot: number
  cot_test?: boolean
  cot_pn?: boolean
  common_address: number
  ioa?: number
  qoi?: number; qcc?: number; qu?: number
  qrp?: number; qpa?: number; qpm?: number
  select?: boolean
  scs?: boolean; dcs?: number; rcs?: number
  nva?: number; sva?: number; r32?: number
  delay_ms?: number
  time?: { year: number; month: number; day: number; hour: number; minute: number; second: number; millis: number; dow: number; dst: boolean; iv: boolean }
}
```

## 测试

后端：
- `iec104sim-core` 给 8 个新 builder 写 hex assertion（参考既有 `test_build_gi_command`），覆盖 act/deact/test/PN 位组合。
- `iec104master-app` 给 `send_asdu` 写 round-trip 测试：mock 一个 stream，比对发出的字节。

前端：
- `npm run build` 必须过。
- Playwright 无头：打开"发送报文"对话框，循环切 17 种 TypeID 截图，验证启用字段；输入一组 C_SC_NA_1 act 并发送，观察是否调到 `invoke('send_asdu')`。

## 兼容性

- 旧配置：`auto_gi_on_connect` 缺省 false（与新建连接默认行为一致），不会触发 GI；用户可手动勾选恢复"老主站习惯"。
- 老主站行为变化：之前默认连接后自动 GI，现改为默认不发。**会影响**已经依赖该行为的用户 → 在 CHANGELOG.md 显著说明，并给"连接后自动总召唤"复选默认 false 的兜底。

## 风险与缓解

- **builder 新签名破坏调用方**：周期性 GI / 现有 `send_interrogation` 等内部调用都得跟着改。统一加 `cot` 参数会让现有调用点都得 patch；先做内部参数化，再暴露到 send_asdu，逐步迁移。
- **17 种 TypeID 测试覆盖**：unit test 写不到所有 COT/字段组合。优先覆盖最容易写错的字节序（CA / IOA / CP56 little-endian）+ QOI/QCC 单字节。Playwright 端做"全列表能下发且不报错"的 smoke。
- **iCloud 异步还原**：编辑后第一时间 commit；前端 `npm run build` 须先做（CLAUDE memory: `shared_frontend_tauri_imports`）。
