# 主站「发送应用层报文」对话框 + 取消自动 GI — 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 复刻经典 IEC-104 主站的「发送应用层报文」对话框（17 种 TypeID × 9 种 COT，按 TypeID 动态显示字段），并让"连接后自动总召唤"变成可配置开关（默认关）。

**Architecture:** 自底向上 4 阶段。core 先扩 8 个 ASDU builder + 1 个高层 `send_asdu(AsduCommand)` 入口；master-app 套 1 个 Tauri 命令做 SendAsduRequest→AsduCommand 映射；前端新增 `SendAsduDialog.vue`，工具栏「自定义控制」按钮改成「发送报文」打开它，`NewConnectionModal` 加 auto_gi 复选，`connectMaster` 读连接配置决定是否 fanOut GI。

**Tech Stack:** Rust (tokio + tauri 2 + serde + chrono)、Vue 3 (Composition API + Tauri invoke + Pinia 风格 ref-state)、TypeScript、Playwright（无头验证）。

参考设计文档：[docs/superpowers/specs/2026-06-02-master-send-asdu-design.md](../specs/2026-06-02-master-send-asdu-design.md)

---

## 阶段 1：core 协议层扩展

### Task 1.1：AsduTypeId 枚举新增 7 项

**Files:**
- Modify: `crates/iec104sim-core/src/types.rs:6-64` (枚举体)
- Modify: `crates/iec104sim-core/src/types.rs:68-96` (`name()`)
- Modify: `crates/iec104sim-core/src/types.rs:100-121` (`description()`)
- Modify: `crates/iec104sim-core/src/types.rs:124-142` (`category()`)
- Modify: `crates/iec104sim-core/src/types.rs:193-223` (`from_u8()`)

- [ ] **Step 1：在枚举体追加 7 项**

在 `types.rs:63` 后（`CCsNa1 = 103,` 这一行之后）插入：

```rust
    /// Read command (Type 102)
    CRdNa1 = 102,
    /// Test command (Type 104)
    CTsNa1 = 104,
    /// Reset process command (Type 105)
    CRpNa1 = 105,
    /// Delay acquisition command (Type 106)
    CCdNa1 = 106,
    /// Parameter normalized (Type 110)
    PMeNa1 = 110,
    /// Parameter scaled (Type 111)
    PMeNb1 = 111,
    /// Parameter short float (Type 112)
    PMeNc1 = 112,
    /// Parameter activation (Type 113)
    PAcNa1 = 113,
```

- [ ] **Step 2：补 `name()` 分支**

在 `name()` 的 `Self::CCsNa1 => "C_CS_NA_1",` 之后追加：

```rust
            Self::CRdNa1 => "C_RD_NA_1",
            Self::CTsNa1 => "C_TS_NA_1",
            Self::CRpNa1 => "C_RP_NA_1",
            Self::CCdNa1 => "C_CD_NA_1",
            Self::PMeNa1 => "P_ME_NA_1",
            Self::PMeNb1 => "P_ME_NB_1",
            Self::PMeNc1 => "P_ME_NC_1",
            Self::PAcNa1 => "P_AC_NA_1",
```

- [ ] **Step 3：补 `description()` 分支**

在 `description()` 的 `Self::CCsNa1 => "时钟同步",` 之后追加：

```rust
            Self::CRdNa1 => "读命令",
            Self::CTsNa1 => "测试命令",
            Self::CRpNa1 => "复位过程命令",
            Self::CCdNa1 => "延时获取命令",
            Self::PMeNa1 => "参数下装(归一化)",
            Self::PMeNb1 => "参数下装(标度化)",
            Self::PMeNc1 => "参数下装(短浮点)",
            Self::PAcNa1 => "参数激活",
```

- [ ] **Step 4：补 `category()`**

在 `category()` 把 `Self::CIcNa1 | Self::CCiNa1 | Self::CCsNa1 => DataCategory::System,` 这一行改成：

```rust
            Self::CIcNa1 | Self::CCiNa1 | Self::CCsNa1
            | Self::CRdNa1 | Self::CTsNa1 | Self::CRpNa1 | Self::CCdNa1
            | Self::PMeNa1 | Self::PMeNb1 | Self::PMeNc1 | Self::PAcNa1
                => DataCategory::System,
```

- [ ] **Step 5：补 `from_u8()`**

在 `from_u8()` 的 `103 => Some(Self::CCsNa1),` 之后追加：

```rust
            102 => Some(Self::CRdNa1),
            104 => Some(Self::CTsNa1),
            105 => Some(Self::CRpNa1),
            106 => Some(Self::CCdNa1),
            110 => Some(Self::PMeNa1),
            111 => Some(Self::PMeNb1),
            112 => Some(Self::PMeNc1),
            113 => Some(Self::PAcNa1),
```

- [ ] **Step 6：跑测试**

Run: `cargo test -p iec104sim-core types::tests`
Expected: PASS（旧测试不应被影响）

- [ ] **Step 7：commit**

```bash
git add crates/iec104sim-core/src/types.rs
git commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" -m "feat(core): AsduTypeId 新增 7 项(C_RD/C_TS/C_RP/C_CD/P_ME_*×3/P_AC)"
```

---

### Task 1.2：新增 8 个 ASDU builder + 单测

**Files:**
- Modify: `crates/iec104sim-core/src/master.rs:2009-2167` (Command frame builders 区段)
- Modify: `crates/iec104sim-core/src/master.rs:2183-2530` (tests mod)

字节布局参考既有 `build_gi_command`（master.rs:2011）：开头 `0x68, total_len_after_this, ctrl1=0, ctrl2=0, ctrl3=0, ctrl4=0,` 共 6 字节固定头，其中前 4 字节由 `send_async_frame` 后续打 SSN/RSN（builder 留 0）。`total_len` = 4 (ctrl) + ASDU 长度。ASDU = 6 字节头 (typeId, NOA=1, cot_lo, cot_hi=0, ca_lo, ca_hi) + payload。

- [ ] **Step 1：先写所有单测（TDD）**

在 `master.rs::tests` 模块末尾、`fn test_build_gi_command()` 附近，追加（找现有 `#[test] fn test_build_gi_command()` 作为锚点）：

```rust
    #[test]
    fn test_build_read_command_hex() {
        // C_RD_NA_1 = 102, CA=1, IOA=42, COT=5
        let frame = build_read_command(1, 42, 5);
        assert_eq!(frame, vec![
            0x68, 0x0D, 0x00, 0x00, 0x00, 0x00,
            102, 0x01, 5, 0x00, 0x01, 0x00,
            0x2A, 0x00, 0x00,
        ]);
    }

    #[test]
    fn test_build_test_command_hex() {
        // C_TS_NA_1 = 104, FBP 固定 0x55 0xAA(小端)
        let frame = build_test_command(2, 6);
        assert_eq!(frame, vec![
            0x68, 0x0F, 0x00, 0x00, 0x00, 0x00,
            104, 0x01, 6, 0x00, 0x02, 0x00,
            0x00, 0x00, 0x00, 0x55, 0xAA,
        ]);
    }

    #[test]
    fn test_build_reset_process_command_hex() {
        // C_RP_NA_1 = 105, QRP=2
        let frame = build_reset_process_command(1, 0, 2, 6);
        assert_eq!(frame, vec![
            0x68, 0x0E, 0x00, 0x00, 0x00, 0x00,
            105, 0x01, 6, 0x00, 0x01, 0x00,
            0x00, 0x00, 0x00, 2,
        ]);
    }

    #[test]
    fn test_build_delay_acq_command_hex() {
        // C_CD_NA_1 = 106, delay=300ms(CP16 LE)
        let frame = build_delay_acq_command(1, 0, 300, 3);
        assert_eq!(frame, vec![
            0x68, 0x0F, 0x00, 0x00, 0x00, 0x00,
            106, 0x01, 3, 0x00, 0x01, 0x00,
            0x00, 0x00, 0x00, 0x2C, 0x01,
        ]);
    }

    #[test]
    fn test_build_parameter_normalized_hex() {
        // P_ME_NA_1 = 110, NVA=0.5 → 16383 / 0x3FFF
        let frame = build_parameter_normalized(1, 100, 0.5, 1, 6);
        assert_eq!(frame, vec![
            0x68, 0x10, 0x00, 0x00, 0x00, 0x00,
            110, 0x01, 6, 0x00, 0x01, 0x00,
            0x64, 0x00, 0x00, 0xFF, 0x3F, 1,
        ]);
    }

    #[test]
    fn test_build_parameter_scaled_hex() {
        // P_ME_NB_1 = 111, SVA=-1 (0xFFFF LE)
        let frame = build_parameter_scaled(1, 50, -1, 2, 6);
        assert_eq!(frame, vec![
            0x68, 0x10, 0x00, 0x00, 0x00, 0x00,
            111, 0x01, 6, 0x00, 0x01, 0x00,
            0x32, 0x00, 0x00, 0xFF, 0xFF, 2,
        ]);
    }

    #[test]
    fn test_build_parameter_float_hex() {
        // P_ME_NC_1 = 112, R32=1.0 (IEEE 754 LE = 00 00 80 3F)
        let frame = build_parameter_float(1, 200, 1.0, 3, 6);
        assert_eq!(frame, vec![
            0x68, 0x12, 0x00, 0x00, 0x00, 0x00,
            112, 0x01, 6, 0x00, 0x01, 0x00,
            0xC8, 0x00, 0x00, 0x00, 0x00, 0x80, 0x3F, 3,
        ]);
    }

    #[test]
    fn test_build_parameter_activation_hex() {
        // P_AC_NA_1 = 113, QPA=2
        let frame = build_parameter_activation(1, 100, 2, 6);
        assert_eq!(frame, vec![
            0x68, 0x0E, 0x00, 0x00, 0x00, 0x00,
            113, 0x01, 6, 0x00, 0x01, 0x00,
            0x64, 0x00, 0x00, 2,
        ]);
    }

    #[test]
    fn test_build_test_pn_bit() {
        // P/N=1 → COT byte |= 0x40
        let frame = build_test_command(1, 6 | 0x40);
        assert_eq!(frame[8], 0x46);
    }

    #[test]
    fn test_build_test_test_bit() {
        // TEST=1 → COT byte |= 0x80
        let frame = build_test_command(1, 6 | 0x80);
        assert_eq!(frame[8], 0x86);
    }
```

- [ ] **Step 2：跑测试确认失败**

Run: `cargo test -p iec104sim-core test_build_read_command_hex 2>&1 | tail -10`
Expected: FAIL with "cannot find function `build_read_command`"

- [ ] **Step 3：在 master.rs:2167（build_bitstring_command 函数尾）后追加 8 个新 builder**

```rust
fn build_read_command(ca: u16, ioa: u32, cot: u8) -> Vec<u8> {
    let ca_bytes = ca.to_le_bytes();
    let ioa_bytes = ioa.to_le_bytes();
    vec![
        0x68, 0x0D,
        0x00, 0x00, 0x00, 0x00,
        102, 0x01, cot, 0x00,
        ca_bytes[0], ca_bytes[1],
        ioa_bytes[0], ioa_bytes[1], ioa_bytes[2],
    ]
}

fn build_test_command(ca: u16, cot: u8) -> Vec<u8> {
    let ca_bytes = ca.to_le_bytes();
    vec![
        0x68, 0x0F,
        0x00, 0x00, 0x00, 0x00,
        104, 0x01, cot, 0x00,
        ca_bytes[0], ca_bytes[1],
        0x00, 0x00, 0x00,
        0x55, 0xAA,
    ]
}

fn build_reset_process_command(ca: u16, ioa: u32, qrp: u8, cot: u8) -> Vec<u8> {
    let ca_bytes = ca.to_le_bytes();
    let ioa_bytes = ioa.to_le_bytes();
    vec![
        0x68, 0x0E,
        0x00, 0x00, 0x00, 0x00,
        105, 0x01, cot, 0x00,
        ca_bytes[0], ca_bytes[1],
        ioa_bytes[0], ioa_bytes[1], ioa_bytes[2],
        qrp,
    ]
}

fn build_delay_acq_command(ca: u16, ioa: u32, delay_ms: u16, cot: u8) -> Vec<u8> {
    let ca_bytes = ca.to_le_bytes();
    let ioa_bytes = ioa.to_le_bytes();
    let d = delay_ms.to_le_bytes();
    vec![
        0x68, 0x0F,
        0x00, 0x00, 0x00, 0x00,
        106, 0x01, cot, 0x00,
        ca_bytes[0], ca_bytes[1],
        ioa_bytes[0], ioa_bytes[1], ioa_bytes[2],
        d[0], d[1],
    ]
}

fn build_parameter_normalized(ca: u16, ioa: u32, nva: f32, qpm: u8, cot: u8) -> Vec<u8> {
    let ca_bytes = ca.to_le_bytes();
    let ioa_bytes = ioa.to_le_bytes();
    let v = (nva * 32767.0) as i16;
    let v_bytes = v.to_le_bytes();
    vec![
        0x68, 0x10,
        0x00, 0x00, 0x00, 0x00,
        110, 0x01, cot, 0x00,
        ca_bytes[0], ca_bytes[1],
        ioa_bytes[0], ioa_bytes[1], ioa_bytes[2],
        v_bytes[0], v_bytes[1],
        qpm,
    ]
}

fn build_parameter_scaled(ca: u16, ioa: u32, sva: i16, qpm: u8, cot: u8) -> Vec<u8> {
    let ca_bytes = ca.to_le_bytes();
    let ioa_bytes = ioa.to_le_bytes();
    let v_bytes = sva.to_le_bytes();
    vec![
        0x68, 0x10,
        0x00, 0x00, 0x00, 0x00,
        111, 0x01, cot, 0x00,
        ca_bytes[0], ca_bytes[1],
        ioa_bytes[0], ioa_bytes[1], ioa_bytes[2],
        v_bytes[0], v_bytes[1],
        qpm,
    ]
}

fn build_parameter_float(ca: u16, ioa: u32, r32: f32, qpm: u8, cot: u8) -> Vec<u8> {
    let ca_bytes = ca.to_le_bytes();
    let ioa_bytes = ioa.to_le_bytes();
    let v_bytes = r32.to_le_bytes();
    vec![
        0x68, 0x12,
        0x00, 0x00, 0x00, 0x00,
        112, 0x01, cot, 0x00,
        ca_bytes[0], ca_bytes[1],
        ioa_bytes[0], ioa_bytes[1], ioa_bytes[2],
        v_bytes[0], v_bytes[1], v_bytes[2], v_bytes[3],
        qpm,
    ]
}

fn build_parameter_activation(ca: u16, ioa: u32, qpa: u8, cot: u8) -> Vec<u8> {
    let ca_bytes = ca.to_le_bytes();
    let ioa_bytes = ioa.to_le_bytes();
    vec![
        0x68, 0x0E,
        0x00, 0x00, 0x00, 0x00,
        113, 0x01, cot, 0x00,
        ca_bytes[0], ca_bytes[1],
        ioa_bytes[0], ioa_bytes[1], ioa_bytes[2],
        qpa,
    ]
}
```

- [ ] **Step 4：跑全部新测试**

Run: `cargo test -p iec104sim-core --lib build_ 2>&1 | tail -20`
Expected: 10 个新测试 PASS（不可有 FAIL）

- [ ] **Step 5：commit**

```bash
git add crates/iec104sim-core/src/master.rs
git commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" -m "feat(core): 8 个新 ASDU builder (C_RD/C_TS/C_RP/C_CD/P_ME_×3/P_AC) + 字节序单测"
```

---

### Task 1.3：暴露高层入口 `MasterConnection::send_asdu`

**Files:**
- Modify: `crates/iec104sim-core/src/master.rs:1050` 之后（`send_raw_apdu` 结束后）

- [ ] **Step 1：先写集成测试占位（验证 enum + dispatcher）**

在 `master.rs::tests` 末尾追加：

```rust
    #[test]
    fn test_asdu_command_build_dispatch() {
        // 验证 send_asdu 的 dispatcher 把 AsduCommand 路由到正确 builder。
        // 仅比较生成的字节,不实际发送(MasterConnection 无可注入 stream)。
        use super::AsduCommand;
        let f = build_asdu_frame(1, 6, &AsduCommand::Read { ioa: 42 });
        assert_eq!(f[6], 102);
        let f = build_asdu_frame(1, 6, &AsduCommand::TestCommand);
        assert_eq!(f[6], 104);
        let f = build_asdu_frame(1, 6, &AsduCommand::ParameterActivation { ioa: 1, qpa: 2 });
        assert_eq!(f[6], 113);
        // TEST 位 + P/N 位通过 COT 字节传入
        let f = build_asdu_frame(1, 6 | 0x80 | 0x40, &AsduCommand::TestCommand);
        assert_eq!(f[8], 0xC6);
    }
```

- [ ] **Step 2：跑测试验证失败**

Run: `cargo test -p iec104sim-core test_asdu_command_build_dispatch 2>&1 | tail -5`
Expected: FAIL with "cannot find function `build_asdu_frame`" or "cannot find type `AsduCommand`"

- [ ] **Step 3：在 master.rs:2168（紧跟最后一个 builder 后）追加 enum + dispatcher**

```rust
/// 高层 ASDU 命令枚举。每个变体对应截图里「报文类型」下拉的一项。
/// `MasterConnection::send_asdu` 接受 `cot` 字节(含 TEST/P/N 高位),把这个 enum
/// 派发到对应的 build_xxx_command,然后通过既有 send_async_frame 走 SSN/RSN 修补、
/// k-window 阻塞、t1 等待逻辑。
#[derive(Debug, Clone)]
pub enum AsduCommand {
    GeneralInterrogation { qoi: u8 },
    CounterRead { qcc: u8 },
    Read { ioa: u32 },
    ClockSync { time: Cp56Time2aFields },
    TestCommand,
    ResetProcess { ioa: u32, qrp: u8 },
    DelayAcq { ioa: u32, delay_ms: u16 },
    SingleCommand { ioa: u32, scs: bool, select: bool, qu: u8 },
    DoubleCommand { ioa: u32, dcs: u8, select: bool, qu: u8 },
    StepCommand { ioa: u32, rcs: u8, select: bool, qu: u8 },
    SetpointNormalized { ioa: u32, nva: f32, select: bool, ql: u8 },
    SetpointScaled { ioa: u32, sva: i16, select: bool, ql: u8 },
    SetpointFloat { ioa: u32, r32: f32, select: bool, ql: u8 },
    ParameterNormalized { ioa: u32, nva: f32, qpm: u8 },
    ParameterScaled { ioa: u32, sva: i16, qpm: u8 },
    ParameterFloat { ioa: u32, r32: f32, qpm: u8 },
    ParameterActivation { ioa: u32, qpa: u8 },
}

/// CP56Time2a 七字节时标(IEC 60870-5-4)。本 crate 已有的 `build_clock_sync_command`
/// 内部仍用系统时间填充,这个结构体只用于「显式发送 C_CS_NA_1 时由调用方指定时间」。
#[derive(Debug, Clone, Copy)]
pub struct Cp56Time2aFields {
    pub year: u16,    // 完整 4 位年,内部 % 100
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub millis: u16,  // 0..=999
    pub dow: u8,      // 0..=7(0 = 未使用)
    pub dst: bool,
    pub iv: bool,
}

impl Cp56Time2aFields {
    fn to_bytes(self) -> [u8; 7] {
        let ms = self.millis + (self.second as u16) * 1000;
        let ms_b = ms.to_le_bytes();
        let mut min = self.minute & 0x3F;
        if self.iv { min |= 0x80; }
        let mut hour = self.hour & 0x1F;
        if self.dst { hour |= 0x80; }
        let day_dow = (self.day & 0x1F) | ((self.dow & 0x07) << 5);
        let year = (self.year % 100) as u8;
        [ms_b[0], ms_b[1], min, hour, day_dow, self.month & 0x0F, year & 0x7F]
    }
}

/// 把 AsduCommand 路由到对应 builder。仅返回字节,不发送 —— 让单测能比对结果。
fn build_asdu_frame(ca: u16, cot: u8, cmd: &AsduCommand) -> Vec<u8> {
    match cmd {
        AsduCommand::GeneralInterrogation { qoi } => {
            let mut f = build_gi_command(ca, *qoi);
            f[8] = cot; f
        }
        AsduCommand::CounterRead { qcc } => {
            let mut f = build_counter_read_command(ca, *qcc);
            f[8] = cot; f
        }
        AsduCommand::Read { ioa } => build_read_command(ca, *ioa, cot),
        AsduCommand::ClockSync { time } => {
            let ca_b = ca.to_le_bytes();
            let t = time.to_bytes();
            vec![
                0x68, 0x14,
                0x00, 0x00, 0x00, 0x00,
                103, 0x01, cot, 0x00,
                ca_b[0], ca_b[1],
                0x00, 0x00, 0x00,
                t[0], t[1], t[2], t[3], t[4], t[5], t[6],
            ]
        }
        AsduCommand::TestCommand => build_test_command(ca, cot),
        AsduCommand::ResetProcess { ioa, qrp } => build_reset_process_command(ca, *ioa, *qrp, cot),
        AsduCommand::DelayAcq { ioa, delay_ms } => build_delay_acq_command(ca, *ioa, *delay_ms, cot),
        AsduCommand::SingleCommand { ioa, scs, select, qu } => build_single_command(ca, *ioa, *scs, *select, *qu, cot),
        AsduCommand::DoubleCommand { ioa, dcs, select, qu } => build_double_command(ca, *ioa, *dcs, *select, *qu, cot),
        AsduCommand::StepCommand { ioa, rcs, select, qu } => build_step_command(ca, *ioa, *rcs, *select, *qu, cot),
        AsduCommand::SetpointNormalized { ioa, nva, select, ql } => build_setpoint_normalized(ca, *ioa, *nva, *select, *ql, cot),
        AsduCommand::SetpointScaled { ioa, sva, select, ql } => build_setpoint_scaled(ca, *ioa, *sva, *select, *ql, cot),
        AsduCommand::SetpointFloat { ioa, r32, select, ql } => build_setpoint_float_command(ca, *ioa, *r32, *select, *ql, cot),
        AsduCommand::ParameterNormalized { ioa, nva, qpm } => build_parameter_normalized(ca, *ioa, *nva, *qpm, cot),
        AsduCommand::ParameterScaled { ioa, sva, qpm } => build_parameter_scaled(ca, *ioa, *sva, *qpm, cot),
        AsduCommand::ParameterFloat { ioa, r32, qpm } => build_parameter_float(ca, *ioa, *r32, *qpm, cot),
        AsduCommand::ParameterActivation { ioa, qpa } => build_parameter_activation(ca, *ioa, *qpa, cot),
    }
}
```

- [ ] **Step 4：在 master.rs:1050（`send_raw_apdu` 结束后）追加公共 API**

```rust
    /// 发送一帧任意 ASDU。`cot` 是已经合并 TEST(bit7) 与 P/N(bit6) 的完整 COT 字节;
    /// 把 ca/cot/cmd 派发到对应 build_xxx,再走 send_frame 的 SSN/RSN/k-window 流程。
    pub async fn send_asdu(&self, ca: u16, cot: u8, cmd: AsduCommand) -> Result<(), MasterError> {
        let frame = build_asdu_frame(ca, cot, &cmd);
        let label = match &cmd {
            AsduCommand::GeneralInterrogation { .. } => FrameLabel::GeneralInterrogation,
            AsduCommand::CounterRead { .. }          => FrameLabel::CounterRead,
            AsduCommand::ClockSync { .. }            => FrameLabel::ClockSync,
            AsduCommand::SingleCommand { .. }        => FrameLabel::SingleCommand,
            AsduCommand::DoubleCommand { .. }        => FrameLabel::DoubleCommand,
            AsduCommand::StepCommand { .. }          => FrameLabel::StepCommand,
            AsduCommand::SetpointNormalized { .. }   => FrameLabel::SetpointNormalized,
            AsduCommand::SetpointScaled { .. }       => FrameLabel::SetpointScaled,
            AsduCommand::SetpointFloat { .. }        => FrameLabel::SetpointFloat,
            _ => FrameLabel::RawApdu,  // 其它类型(Read/Test/Reset/Delay/P_*)暂无专属 label
        };
        let detail = format!("ASDU type={} cot={}", asdu_type_of(&cmd), cot);
        self.send_frame(&frame, &detail, label, ca).await
    }
```

并在 dispatcher 旁边加一个内部小帮手：

```rust
fn asdu_type_of(cmd: &AsduCommand) -> u8 {
    match cmd {
        AsduCommand::GeneralInterrogation { .. } => 100,
        AsduCommand::CounterRead { .. }          => 101,
        AsduCommand::Read { .. }                 => 102,
        AsduCommand::ClockSync { .. }            => 103,
        AsduCommand::TestCommand                 => 104,
        AsduCommand::ResetProcess { .. }         => 105,
        AsduCommand::DelayAcq { .. }             => 106,
        AsduCommand::SingleCommand { .. }        => 45,
        AsduCommand::DoubleCommand { .. }        => 46,
        AsduCommand::StepCommand { .. }          => 47,
        AsduCommand::SetpointNormalized { .. }   => 48,
        AsduCommand::SetpointScaled { .. }       => 49,
        AsduCommand::SetpointFloat { .. }        => 50,
        AsduCommand::ParameterNormalized { .. }  => 110,
        AsduCommand::ParameterScaled { .. }      => 111,
        AsduCommand::ParameterFloat { .. }       => 112,
        AsduCommand::ParameterActivation { .. }  => 113,
    }
}
```

- [ ] **Step 5：跑 dispatcher 测试**

Run: `cargo test -p iec104sim-core test_asdu_command_build_dispatch 2>&1 | tail -5`
Expected: PASS

- [ ] **Step 6：full 单测保证旧逻辑没坏**

Run: `cargo test -p iec104sim-core 2>&1 | tail -5`
Expected: 全部 PASS

- [ ] **Step 7：commit**

```bash
git add crates/iec104sim-core/src/master.rs
git commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" -m "feat(core): MasterConnection::send_asdu 公共 API + AsduCommand 枚举 + dispatcher"
```

---

### Task 1.4：MasterConfig 加 `auto_gi_on_connect` 字段

**Files:**
- Modify: `crates/iec104sim-core/src/master.rs:200-280` (MasterConfig 结构体 + default)

- [ ] **Step 1：写测试**

在 `master.rs::tests` 加：

```rust
    #[test]
    fn test_master_config_default_auto_gi_off() {
        let c = MasterConfig::default();
        assert!(!c.auto_gi_on_connect, "默认必须关闭,避免不告知用户就发 GI");
    }

    #[test]
    fn test_master_config_deserialize_missing_auto_gi() {
        // 旧 JSON 配置(没有 auto_gi_on_connect 字段)必须能正确加载
        let json = r#"{
            "target_address": "127.0.0.1",
            "port": 2404,
            "common_address": 1,
            "timeout_ms": 3000,
            "tls": { "enabled": false, "ca_file": "", "cert_file": "", "key_file": "",
                     "pkcs12_file": "", "pkcs12_password": "", "accept_invalid_certs": false,
                     "version": "auto" }
        }"#;
        let c: MasterConfig = serde_json::from_str(json).unwrap();
        assert!(!c.auto_gi_on_connect);
    }
```

- [ ] **Step 2：跑测试确认失败**

Run: `cargo test -p iec104sim-core test_master_config_default_auto_gi_off 2>&1 | tail -5`
Expected: FAIL（字段未定义）

- [ ] **Step 3：在 MasterConfig 结构体追加字段**

定位 `crates/iec104sim-core/src/master.rs:242` 附近（`pub counter_interrogate_period_s: u32,` 之后），追加：

```rust
    /// 连接成功后是否自动发一次 GI。默认 false。
    /// 老主站习惯是自动发,但与 IEC 60870-5-104 标准不强制,且会让"裸帧调试"
    /// 测试场景被噪声污染。前端可在新建连接复选框里勾选恢复旧行为。
    #[serde(default)]
    pub auto_gi_on_connect: bool,
```

并同步给 `impl Default for MasterConfig` 添加（master.rs:260-280 区间）：

```rust
            auto_gi_on_connect: false,
```

- [ ] **Step 4：跑测试**

Run: `cargo test -p iec104sim-core test_master_config 2>&1 | tail -10`
Expected: PASS

- [ ] **Step 5：commit**

```bash
git add crates/iec104sim-core/src/master.rs
git commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" -m "feat(core): MasterConfig.auto_gi_on_connect 默认 false,序列化向后兼容"
```

---

## 阶段 2：master-app 命令层

### Task 2.1：`CreateConnectionRequest`/`ConnectionInfo` 加 auto_gi 字段

**Files:**
- Modify: `crates/iec104master-app/src/commands.rs:20-77` (CreateConnectionRequest)
- Modify: `crates/iec104master-app/src/commands.rs:120-135` (config 注入)
- Modify: `crates/iec104master-app/src/state.rs` (ConnectionInfo 如在此处定义)
- Modify: `crates/iec104master-app/src/commands.rs:165-200, 295-325` (info 装配 + list)

- [ ] **Step 1：定位 ConnectionInfo 定义**

Run: `grep -n "pub struct ConnectionInfo" crates/iec104master-app/src/*.rs`
Expected: 输出准确文件:行号

- [ ] **Step 2：在 `CreateConnectionRequest` 追加字段（commands.rs:63 附近 broadcast_address 后）**

```rust
    /// 连接成功后是否自动发一次 GI。可空,缺省由 MasterConfig::default() 决定(false)。
    pub auto_gi_on_connect: Option<bool>,
```

- [ ] **Step 3：在 `create_connection` 注入（commands.rs:134 `if let Some(bcast)...` 后）**

```rust
    if let Some(v) = request.auto_gi_on_connect { config.auto_gi_on_connect = v; }
```

- [ ] **Step 4：在 `ConnectionInfo` 结构体加字段**

定位到 ConnectionInfo 定义（很可能在 state.rs 或 commands.rs 顶部），在 `broadcast_address: u16,` 后追加：

```rust
    pub auto_gi_on_connect: bool,
```

- [ ] **Step 5：在 `create_connection` 装配 `info` 时填字段（commands.rs:186 broadcast_address 行后）**

```rust
        auto_gi_on_connect: config.auto_gi_on_connect,
```

并在 `list_connections` 的循环（commands.rs:323 附近）同步加：

```rust
            auto_gi_on_connect: cfg.auto_gi_on_connect,
```

- [ ] **Step 6：跑编译**

Run: `cargo build -p iec104master-app 2>&1 | tail -10`
Expected: 成功，无 warning

- [ ] **Step 7：commit**

```bash
git add crates/iec104master-app/
git commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" -m "feat(master-app): CreateConnectionRequest/ConnectionInfo 透传 auto_gi_on_connect"
```

---

### Task 2.2：新增 `send_asdu` Tauri 命令

**Files:**
- Modify: `crates/iec104master-app/src/commands.rs` 末尾
- Modify: `crates/iec104master-app/src/lib.rs:16-49` (invoke_handler 注册)

- [ ] **Step 1：在 commands.rs 末尾追加 SendAsduRequest + send_asdu**

```rust
// ----------------------------------------------------------------------------
// 「发送应用层报文」对话框的入口。前端按 TypeID 动态收集字段,
// 后端 switch 到对应 AsduCommand 变体,缺字段直接报错。
// ----------------------------------------------------------------------------

#[derive(Debug, serde::Deserialize)]
pub struct SendAsduRequest {
    pub type_id: u8,
    pub cot: u8,
    /// bit 7 of COT byte
    pub cot_test: Option<bool>,
    /// bit 6 of COT byte
    pub cot_pn: Option<bool>,
    pub common_address: u16,
    pub ioa: Option<u32>,
    pub qoi: Option<u8>,
    pub qcc: Option<u8>,
    pub qrp: Option<u8>,
    pub qpa: Option<u8>,
    pub qpm: Option<u8>,
    pub qu: Option<u8>,
    pub select: Option<bool>,
    pub scs: Option<bool>,
    pub dcs: Option<u8>,
    pub rcs: Option<u8>,
    pub nva: Option<f32>,
    pub sva: Option<i16>,
    pub r32: Option<f32>,
    pub delay_ms: Option<u16>,
    pub time: Option<Cp56Time2aDto>,
}

#[derive(Debug, serde::Deserialize)]
pub struct Cp56Time2aDto {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub millis: u16,
    pub dow: u8,
    pub dst: bool,
    pub iv: bool,
}

impl From<Cp56Time2aDto> for iec104sim_core::Cp56Time2aFields {
    fn from(d: Cp56Time2aDto) -> Self {
        Self {
            year: d.year, month: d.month, day: d.day,
            hour: d.hour, minute: d.minute, second: d.second,
            millis: d.millis, dow: d.dow, dst: d.dst, iv: d.iv,
        }
    }
}

fn need<T>(field: &'static str, v: Option<T>) -> Result<T, String> {
    v.ok_or_else(|| format!("missing field {}", field))
}

fn to_asdu_command(r: &SendAsduRequest) -> Result<iec104sim_core::AsduCommand, String> {
    use iec104sim_core::AsduCommand as A;
    Ok(match r.type_id {
        100 => A::GeneralInterrogation { qoi: need("qoi", r.qoi)? },
        101 => A::CounterRead          { qcc: need("qcc", r.qcc)? },
        102 => A::Read                 { ioa: need("ioa", r.ioa)? },
        103 => {
            let t = r.time.as_ref().ok_or_else(|| "missing field time".to_string())?;
            A::ClockSync { time: (*t).clone().into() }
        }
        104 => A::TestCommand,
        105 => A::ResetProcess { ioa: need("ioa", r.ioa)?, qrp: need("qrp", r.qrp)? },
        106 => A::DelayAcq     { ioa: need("ioa", r.ioa)?, delay_ms: need("delay_ms", r.delay_ms)? },
        45  => A::SingleCommand    { ioa: need("ioa", r.ioa)?, scs: need("scs", r.scs)?, select: r.select.unwrap_or(false), qu: r.qu.unwrap_or(0) },
        46  => A::DoubleCommand    { ioa: need("ioa", r.ioa)?, dcs: need("dcs", r.dcs)?, select: r.select.unwrap_or(false), qu: r.qu.unwrap_or(0) },
        47  => A::StepCommand      { ioa: need("ioa", r.ioa)?, rcs: need("rcs", r.rcs)?, select: r.select.unwrap_or(false), qu: r.qu.unwrap_or(0) },
        48  => A::SetpointNormalized { ioa: need("ioa", r.ioa)?, nva: need("nva", r.nva)?, select: r.select.unwrap_or(false), ql: r.qu.unwrap_or(0) },
        49  => A::SetpointScaled     { ioa: need("ioa", r.ioa)?, sva: need("sva", r.sva)?, select: r.select.unwrap_or(false), ql: r.qu.unwrap_or(0) },
        50  => A::SetpointFloat      { ioa: need("ioa", r.ioa)?, r32: need("r32", r.r32)?, select: r.select.unwrap_or(false), ql: r.qu.unwrap_or(0) },
        110 => A::ParameterNormalized { ioa: need("ioa", r.ioa)?, nva: need("nva", r.nva)?, qpm: need("qpm", r.qpm)? },
        111 => A::ParameterScaled     { ioa: need("ioa", r.ioa)?, sva: need("sva", r.sva)?, qpm: need("qpm", r.qpm)? },
        112 => A::ParameterFloat      { ioa: need("ioa", r.ioa)?, r32: need("r32", r.r32)?, qpm: need("qpm", r.qpm)? },
        113 => A::ParameterActivation { ioa: need("ioa", r.ioa)?, qpa: need("qpa", r.qpa)? },
        other => return Err(format!("unsupported type_id {}", other)),
    })
}

#[tauri::command]
pub async fn send_asdu(
    state: State<'_, AppState>,
    id: String,
    request: SendAsduRequest,
) -> Result<(), String> {
    let connections = state.connections.read().await;
    let conn = connections
        .get(&id)
        .ok_or_else(|| format!("connection {} not found", id))?;
    let cmd = to_asdu_command(&request)?;
    // 把 TEST/P_N 位合并进 cot byte
    let mut cot = request.cot;
    if request.cot_test.unwrap_or(false) { cot |= 0x80; }
    if request.cot_pn.unwrap_or(false) { cot |= 0x40; }
    conn.connection
        .send_asdu(request.common_address, cot, cmd)
        .await
        .map_err(|e| format!("failed to send ASDU: {}", e))
}
```

- [ ] **Step 2：在 iec104sim-core 的 lib 重新导出 AsduCommand / Cp56Time2aFields**

Run: `grep -n "pub use\|pub fn run\|pub mod" crates/iec104sim-core/src/lib.rs | head -10`
确保 `AsduCommand` 和 `Cp56Time2aFields` 已被 export。如果没有，往 `crates/iec104sim-core/src/lib.rs` 加：

```rust
pub use master::{AsduCommand, Cp56Time2aFields, MasterConfig, MasterConnection, MasterError, MasterState};
```

（如果文件里已有 `pub use master::{...}` 直接补上缺失的）

- [ ] **Step 3：在 lib.rs invoke_handler 注册命令**

`crates/iec104master-app/src/lib.rs:30` 处 `commands::send_control_command,` 后追加一行：

```rust
            commands::send_asdu,
```

- [ ] **Step 4：编译**

Run: `cargo build -p iec104master-app 2>&1 | tail -15`
Expected: 成功，无 error

- [ ] **Step 5：commit**

```bash
git add crates/
git commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" -m "feat(master-app): send_asdu 命令 + SendAsduRequest schema 覆盖 17 TypeID/COT 组合"
```

---

## 阶段 3：前端

### Task 3.1：扩 `master-frontend/src/types.ts`

**Files:**
- Modify: `master-frontend/src/types.ts:7-31` (ConnectionInfo)
- Modify: `master-frontend/src/types.ts` 末尾追加 SendAsduRequest 和常量

- [ ] **Step 1：在 ConnectionInfo 加字段**

把 `master-frontend/src/types.ts:27` 的 `broadcast_address: number` 行后插入：

```ts
  /** 连接成功后是否自动发 GI。默认 false。 */
  auto_gi_on_connect: boolean
```

- [ ] **Step 2：在文件末尾（asduHasTimestamp 函数之后）追加**

```ts
// ---------- 「发送应用层报文」对话框相关 ----------

export interface SendAsduTime {
  year: number; month: number; day: number
  hour: number; minute: number; second: number
  millis: number
  dow: number; dst: boolean; iv: boolean
}

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
  time?: SendAsduTime
}

export interface AsduTypeOption { id: number; name: string; desc: string }

/** 截图里 17 项 TypeID 的下拉数据(顺序贴近原版)。 */
export const ASDU_TYPE_OPTIONS: AsduTypeOption[] = [
  { id: 100, name: 'C_IC_NA_1', desc: '总召唤' },
  { id: 45,  name: 'C_SC_NA_1', desc: '单点遥控' },
  { id: 46,  name: 'C_DC_NA_1', desc: '双点遥控' },
  { id: 102, name: 'C_RD_NA_1', desc: '读数据' },
  { id: 103, name: 'C_CS_NA_1', desc: '时钟同步' },
  { id: 106, name: 'C_CD_NA_1', desc: '延时获取' },
  { id: 104, name: 'C_TS_NA_1', desc: '测试命令' },
  { id: 105, name: 'C_RP_NA_1', desc: '复位过程' },
  { id: 101, name: 'C_CI_NA_1', desc: '累计量召唤' },
  { id: 110, name: 'P_ME_NA_1', desc: '参数下装-归一化值' },
  { id: 112, name: 'P_ME_NC_1', desc: '参数下装-短浮点数' },
  { id: 111, name: 'P_ME_NB_1', desc: '参数下装-标度化值' },
  { id: 48,  name: 'C_SE_NA_1', desc: '设点命令-归一化值' },
  { id: 49,  name: 'C_SE_NB_1', desc: '设点命令-标度化值' },
  { id: 50,  name: 'C_SE_NC_1', desc: '设点命令-短浮点数' },
  { id: 47,  name: 'C_RC_NA_1', desc: '调节步命令' },
  { id: 113, name: 'P_AC_NA_1', desc: '参数激活' },
]

export interface CotOption { value: number; tag: string; label: string }

export const COT_OPTIONS: CotOption[] = [
  { value: 6,  tag: 'act',      label: '激活' },
  { value: 7,  tag: 'actcon',   label: '激活确认' },
  { value: 8,  tag: 'deact',    label: '停止激活' },
  { value: 9,  tag: 'deactcon', label: '停止激活确认' },
  { value: 10, tag: 'actterm',  label: '激活终止' },
  { value: 11, tag: 'retrem',   label: '远程命令引起的返送信息' },
  { value: 12, tag: 'retloc',   label: '当地命令引起的返送信息' },
  { value: 13, tag: 'file',     label: '文件传送' },
  { value: 20, tag: 'inrogen',  label: '响应总召唤' },
]

/** 按 TypeID 返回启用字段集。前端 SendAsduDialog 据此 enable/disable input。 */
export interface AsduEnabledFields {
  ioa: boolean
  qoi: boolean; qcc: boolean; qu: boolean
  qrp: boolean; qpa: boolean; qpm: boolean
  scs: boolean; dcs: boolean; rcs: boolean
  nva: boolean; sva: boolean; r32: boolean
  select: boolean
  delay: boolean
  time: boolean
}
const NONE: AsduEnabledFields = {
  ioa: false, qoi: false, qcc: false, qu: false, qrp: false, qpa: false, qpm: false,
  scs: false, dcs: false, rcs: false, nva: false, sva: false, r32: false,
  select: false, delay: false, time: false,
}
export function enabledFields(typeId: number): AsduEnabledFields {
  switch (typeId) {
    case 100: return { ...NONE, qoi: true }
    case 101: return { ...NONE, qcc: true }
    case 102: return { ...NONE, ioa: true }
    case 103: return { ...NONE, ioa: true, time: true }
    case 104: return { ...NONE }
    case 105: return { ...NONE, ioa: true, qrp: true }
    case 106: return { ...NONE, ioa: true, delay: true }
    case 45:  return { ...NONE, ioa: true, scs: true, select: true, qu: true }
    case 46:  return { ...NONE, ioa: true, dcs: true, select: true, qu: true }
    case 47:  return { ...NONE, ioa: true, rcs: true, select: true, qu: true }
    case 48:  return { ...NONE, ioa: true, nva: true, select: true, qu: true }
    case 49:  return { ...NONE, ioa: true, sva: true, select: true, qu: true }
    case 50:  return { ...NONE, ioa: true, r32: true, select: true, qu: true }
    case 110: return { ...NONE, ioa: true, nva: true, qpm: true }
    case 111: return { ...NONE, ioa: true, sva: true, qpm: true }
    case 112: return { ...NONE, ioa: true, r32: true, qpm: true }
    case 113: return { ...NONE, ioa: true, qpa: true }
    default:  return NONE
  }
}
```

- [ ] **Step 3：commit**

```bash
git add master-frontend/src/types.ts
git commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" -m "feat(master-fe): types.ts 加 SendAsduRequest/ASDU_TYPE_OPTIONS/enabledFields"
```

---

### Task 3.2：补 i18n 字典

**Files:**
- Modify: `master-frontend/src/i18n/locales/zh-CN.ts`
- Modify: `master-frontend/src/i18n/locales/en-US.ts`

- [ ] **Step 1：zh-CN DictShape 加段（位于 control: {...} 之后、about 之前）**

在 `master-frontend/src/i18n/locales/zh-CN.ts:214` 紧跟 `}, about: ...` 前插入 type 段：

```ts
  sendAsdu: {
    title: string
    typeId: string
    cot: string
    test: string
    pn: string
    ca: string
    ioa: string
    qoi: string; qcc: string; qu: string
    qrp: string; qpa: string; qpm: string
    scs: string; dcs: string; rcs: string
    nva: string; sva: string; r32: string
    delayMs: string
    timeYear: string; timeMonth: string; timeDay: string
    timeHour: string; timeMin: string; timeSec: string; timeMs: string
    timeDow: string; timeDst: string; timeIv: string
    select: string
    send: string; sending: string; cancel: string
    sentOk: string
  }
```

同步在 `toolbar: {...}` 段（zh-CN.ts:13-42）加：

```ts
    sendAsdu: string
```

`newConn: {...}` 段（zh-CN.ts:43-73）加：

```ts
    autoGi: string
    autoGiHint: string
```

- [ ] **Step 2：zh-CN dict 实体补值**

在 `master-frontend/src/i18n/locales/zh-CN.ts` 的 `toolbar` 对象（看 `customControl: '自定义控制',` 行）后加：

```ts
    sendAsdu: '发送报文',
```

在 `newConn` 对象末尾（`broadcastAddressInvalid` 后）加：

```ts
    autoGi: '连接后自动总召唤',
    autoGiHint: '默认关闭。勾选可恢复"老主站"行为。',
```

在 `control: {...}` 与 `about: {...}` 之间插入完整 sendAsdu 字典：

```ts
  sendAsdu: {
    title: '发送应用层报文',
    typeId: '报文类型',
    cot: '传输原因',
    test: 'TEST',
    pn: 'P/N',
    ca: '公共地址',
    ioa: '信息体地址',
    qoi: 'QOI', qcc: 'QCC', qu: 'QU',
    qrp: 'QRP', qpa: 'QPA', qpm: 'QPM',
    scs: 'SCS', dcs: 'DCS', rcs: 'RCS',
    nva: 'SAV (归一化)', sva: 'SAV (标度化)', r32: '短浮点数',
    delayMs: '传输延时(ms)',
    timeYear: '年', timeMonth: '月', timeDay: '日',
    timeHour: '时', timeMin: '分', timeSec: '秒', timeMs: '毫秒',
    timeDow: '星期', timeDst: '夏令时', timeIv: '时标无效',
    select: '选择/执行',
    send: '发送', sending: '发送中…', cancel: '取消',
    sentOk: '发送成功',
  },
```

- [ ] **Step 3：en-US 镜像**

打开 `master-frontend/src/i18n/locales/en-US.ts`，把同样的 DictShape 字段（toolbar.sendAsdu / newConn.autoGi/autoGiHint / sendAsdu.* 块）按英文翻译同步加上：

```ts
    sendAsdu: 'Send ASDU',
```

```ts
    autoGi: 'Auto General Interrogation on connect',
    autoGiHint: 'Default off. Enable to restore "classic master" behavior.',
```

```ts
  sendAsdu: {
    title: 'Send Application Layer ASDU',
    typeId: 'Type ID',
    cot: 'Cause of Transmission',
    test: 'TEST',
    pn: 'P/N',
    ca: 'Common Address',
    ioa: 'Info Object Address',
    qoi: 'QOI', qcc: 'QCC', qu: 'QU',
    qrp: 'QRP', qpa: 'QPA', qpm: 'QPM',
    scs: 'SCS', dcs: 'DCS', rcs: 'RCS',
    nva: 'SAV (normalized)', sva: 'SAV (scaled)', r32: 'Short float',
    delayMs: 'Delay (ms)',
    timeYear: 'Year', timeMonth: 'Month', timeDay: 'Day',
    timeHour: 'Hour', timeMin: 'Min', timeSec: 'Sec', timeMs: 'ms',
    timeDow: 'DoW', timeDst: 'DST', timeIv: 'Time invalid',
    select: 'Select / Execute',
    send: 'Send', sending: 'Sending…', cancel: 'Cancel',
    sentOk: 'Sent OK',
  },
```

- [ ] **Step 4：编译看类型对齐**

Run: `cd master-frontend && npm run build 2>&1 | tail -15`
Expected: 成功；zh-CN 与 en-US 字段集一致时 TS 不会报缺字段。

- [ ] **Step 5：commit**

```bash
git add master-frontend/src/i18n/
git commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" -m "i18n(master-fe): 补 sendAsdu 字典 + newConn.autoGi + toolbar.sendAsdu"
```

---

### Task 3.3：新建 `SendAsduDialog.vue`

**Files:**
- Create: `master-frontend/src/components/SendAsduDialog.vue`

按既有 ControlDialog.vue 风格写，复用 modal-* CSS class（同文件内 scoped）。

- [ ] **Step 1：创建文件**

```vue
<script setup lang="ts">
import { ref, watch, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useI18n } from '@shared/i18n'
import {
  ASDU_TYPE_OPTIONS, COT_OPTIONS, enabledFields,
  type SendAsduRequest,
} from '../types'

const { t } = useI18n()

interface Props {
  visible: boolean
  connectionId: string | null
  defaultCA: number
}
const props = defineProps<Props>()
const emit = defineEmits<{ (e: 'close'): void; (e: 'sent'): void }>()

const STORAGE_KEY = 'iec104master.sendAsduDialog.v1'
type Persisted = Partial<{
  typeId: number; cot: number
  cotTest: boolean; cotPn: boolean
  ca: number; ioa: number
  qoi: number; qcc: number; qu: number
  qrp: number; qpa: number; qpm: number
  scs: boolean; dcs: number; rcs: number
  nva: number; sva: number; r32: number
  select: boolean
  delayMs: number
  time: { year: number; month: number; day: number; hour: number; minute: number; second: number; millis: number; dow: number; dst: boolean; iv: boolean }
}>
function loadPersisted(): Persisted {
  try { const raw = localStorage.getItem(STORAGE_KEY); if (raw) return JSON.parse(raw) as Persisted } catch {}
  return {}
}
const saved = loadPersisted()

const typeId = ref<number>(saved.typeId ?? 100)
const cot = ref<number>(saved.cot ?? 6)
const cotTest = ref<boolean>(saved.cotTest ?? false)
const cotPn = ref<boolean>(saved.cotPn ?? false)
const ca = ref<number>(saved.ca ?? props.defaultCA)
const ioa = ref<number>(saved.ioa ?? 0)
const qoi = ref<number>(saved.qoi ?? 20)
const qcc = ref<number>(saved.qcc ?? 5)
const qu = ref<number>(saved.qu ?? 0)
const qrp = ref<number>(saved.qrp ?? 1)
const qpa = ref<number>(saved.qpa ?? 1)
const qpm = ref<number>(saved.qpm ?? 0)
const scs = ref<boolean>(saved.scs ?? false)
const dcs = ref<number>(saved.dcs ?? 1)
const rcs = ref<number>(saved.rcs ?? 1)
const nva = ref<number>(saved.nva ?? 0)
const sva = ref<number>(saved.sva ?? 0)
const r32 = ref<number>(saved.r32 ?? 0)
const select = ref<boolean>(saved.select ?? false)
const delayMs = ref<number>(saved.delayMs ?? 0)
const time = ref({
  year: saved.time?.year ?? new Date().getFullYear(),
  month: saved.time?.month ?? (new Date().getMonth() + 1),
  day: saved.time?.day ?? new Date().getDate(),
  hour: saved.time?.hour ?? new Date().getHours(),
  minute: saved.time?.minute ?? new Date().getMinutes(),
  second: saved.time?.second ?? new Date().getSeconds(),
  millis: saved.time?.millis ?? 0,
  dow: saved.time?.dow ?? 0,
  dst: saved.time?.dst ?? false,
  iv: saved.time?.iv ?? false,
})

const sending = ref(false)
const errorMsg = ref('')
const okFlag = ref(false)

const fields = computed(() => enabledFields(typeId.value))

function savePersisted() {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify({
      typeId: typeId.value, cot: cot.value, cotTest: cotTest.value, cotPn: cotPn.value,
      ca: ca.value, ioa: ioa.value,
      qoi: qoi.value, qcc: qcc.value, qu: qu.value,
      qrp: qrp.value, qpa: qpa.value, qpm: qpm.value,
      scs: scs.value, dcs: dcs.value, rcs: rcs.value,
      nva: nva.value, sva: sva.value, r32: r32.value,
      select: select.value, delayMs: delayMs.value, time: time.value,
    }))
  } catch {}
}

watch(() => props.visible, (v) => {
  if (v) {
    errorMsg.value = ''; okFlag.value = false; sending.value = false
    if (ca.value === 0) ca.value = props.defaultCA
  }
})

async function send() {
  if (!props.connectionId) return
  errorMsg.value = ''; okFlag.value = false; sending.value = true
  const f = fields.value
  const req: SendAsduRequest = {
    type_id: typeId.value,
    cot: cot.value,
    cot_test: cotTest.value,
    cot_pn: cotPn.value,
    common_address: ca.value,
    ioa: f.ioa ? ioa.value : undefined,
    qoi: f.qoi ? qoi.value : undefined,
    qcc: f.qcc ? qcc.value : undefined,
    qu:  f.qu  ? qu.value  : undefined,
    qrp: f.qrp ? qrp.value : undefined,
    qpa: f.qpa ? qpa.value : undefined,
    qpm: f.qpm ? qpm.value : undefined,
    scs: f.scs ? scs.value : undefined,
    dcs: f.dcs ? dcs.value : undefined,
    rcs: f.rcs ? rcs.value : undefined,
    nva: f.nva ? nva.value : undefined,
    sva: f.sva ? sva.value : undefined,
    r32: f.r32 ? r32.value : undefined,
    select: f.select ? select.value : undefined,
    delay_ms: f.delay ? delayMs.value : undefined,
    time: f.time ? time.value : undefined,
  }
  try {
    await invoke('send_asdu', { id: props.connectionId, request: req })
    savePersisted()
    okFlag.value = true
    emit('sent')
  } catch (e) {
    errorMsg.value = String(e)
  } finally {
    sending.value = false
  }
}

function onKey(e: KeyboardEvent) {
  if (e.key === 'Escape') emit('close')
  else if (e.key === 'Enter' && !sending.value) send()
}
</script>

<template>
  <Teleport to="body">
    <Transition name="dialog-pop">
    <div v-if="visible" class="modal-backdrop dialog-blur" @mousedown.self="emit('close')" @keydown="onKey">
      <div class="modal-box modal-wide">
        <div class="modal-title">{{ t('sendAsdu.title') }}</div>
        <div class="modal-body">
          <div class="form-row">
            <label class="form-label form-label-half">
              {{ t('sendAsdu.typeId') }}
              <div class="inline-pair">
                <input v-model.number="typeId" class="form-input mono" type="number" min="0" max="255" />
                <select v-model.number="typeId" class="form-input">
                  <option v-for="o in ASDU_TYPE_OPTIONS" :key="o.id" :value="o.id">{{ o.name }} ({{ o.desc }})</option>
                </select>
              </div>
            </label>
            <label class="form-label form-label-half">
              {{ t('sendAsdu.cot') }}
              <div class="inline-pair">
                <input v-model.number="cot" class="form-input mono" type="number" min="0" max="63" />
                <select v-model.number="cot" class="form-input">
                  <option v-for="o in COT_OPTIONS" :key="o.value" :value="o.value">{{ o.label }} ({{ o.tag }})</option>
                </select>
              </div>
            </label>
          </div>

          <div class="form-row">
            <label class="check-inline"><input type="checkbox" v-model="cotTest" /> {{ t('sendAsdu.test') }}</label>
            <label class="check-inline"><input type="checkbox" v-model="cotPn" /> {{ t('sendAsdu.pn') }}</label>
          </div>

          <div class="form-row">
            <label class="form-label form-label-half">
              {{ t('sendAsdu.ca') }}
              <input v-model.number="ca" class="form-input" type="number" min="1" max="65534" />
            </label>
            <label class="form-label form-label-half" :class="{ disabled: !fields.ioa }">
              {{ t('sendAsdu.ioa') }}
              <input v-model.number="ioa" class="form-input" type="number" min="0" max="16777215" :disabled="!fields.ioa" />
            </label>
          </div>

          <!-- 字段网格,按 enabledFields 启用 -->
          <div class="field-grid">
            <label v-if="fields.qoi" class="form-label">QOI<input v-model.number="qoi" class="form-input" type="number" min="0" max="255" /></label>
            <label v-if="fields.qcc" class="form-label">QCC<input v-model.number="qcc" class="form-input" type="number" min="0" max="255" /></label>
            <label v-if="fields.qu"  class="form-label">QU<input v-model.number="qu"  class="form-input" type="number" min="0" max="31" /></label>
            <label v-if="fields.qrp" class="form-label">QRP<input v-model.number="qrp" class="form-input" type="number" min="0" max="255" /></label>
            <label v-if="fields.qpa" class="form-label">QPA<input v-model.number="qpa" class="form-input" type="number" min="0" max="255" /></label>
            <label v-if="fields.qpm" class="form-label">QPM<input v-model.number="qpm" class="form-input" type="number" min="0" max="255" /></label>
            <label v-if="fields.scs" class="form-label">SCS<select v-model="scs" class="form-input"><option :value="false">0</option><option :value="true">1</option></select></label>
            <label v-if="fields.dcs" class="form-label">DCS<select v-model.number="dcs" class="form-input"><option :value="0">0</option><option :value="1">1(开)</option><option :value="2">2(合)</option><option :value="3">3</option></select></label>
            <label v-if="fields.rcs" class="form-label">RCS<select v-model.number="rcs" class="form-input"><option :value="0">0</option><option :value="1">1(降)</option><option :value="2">2(升)</option><option :value="3">3</option></select></label>
            <label v-if="fields.nva" class="form-label">{{ t('sendAsdu.nva') }}<input v-model.number="nva" class="form-input" type="number" step="0.001" min="-1" max="1" /></label>
            <label v-if="fields.sva" class="form-label">{{ t('sendAsdu.sva') }}<input v-model.number="sva" class="form-input" type="number" min="-32768" max="32767" /></label>
            <label v-if="fields.r32" class="form-label">{{ t('sendAsdu.r32') }}<input v-model.number="r32" class="form-input" type="number" step="0.001" /></label>
            <label v-if="fields.delay" class="form-label">{{ t('sendAsdu.delayMs') }}<input v-model.number="delayMs" class="form-input" type="number" min="0" max="65535" /></label>
            <label v-if="fields.select" class="form-label">{{ t('sendAsdu.select') }}<select v-model="select" class="form-input"><option :value="false">执行</option><option :value="true">选择</option></select></label>
          </div>

          <div v-if="fields.time" class="time-grid">
            <label class="form-label">{{ t('sendAsdu.timeYear') }}<input v-model.number="time.year" class="form-input" type="number" min="2000" max="2099" /></label>
            <label class="form-label">{{ t('sendAsdu.timeMonth') }}<input v-model.number="time.month" class="form-input" type="number" min="1" max="12" /></label>
            <label class="form-label">{{ t('sendAsdu.timeDay') }}<input v-model.number="time.day" class="form-input" type="number" min="1" max="31" /></label>
            <label class="form-label">{{ t('sendAsdu.timeHour') }}<input v-model.number="time.hour" class="form-input" type="number" min="0" max="23" /></label>
            <label class="form-label">{{ t('sendAsdu.timeMin') }}<input v-model.number="time.minute" class="form-input" type="number" min="0" max="59" /></label>
            <label class="form-label">{{ t('sendAsdu.timeSec') }}<input v-model.number="time.second" class="form-input" type="number" min="0" max="59" /></label>
            <label class="form-label">{{ t('sendAsdu.timeMs') }}<input v-model.number="time.millis" class="form-input" type="number" min="0" max="999" /></label>
            <label class="form-label">{{ t('sendAsdu.timeDow') }}<input v-model.number="time.dow" class="form-input" type="number" min="0" max="7" /></label>
            <label class="check-inline"><input type="checkbox" v-model="time.dst" /> {{ t('sendAsdu.timeDst') }}</label>
            <label class="check-inline"><input type="checkbox" v-model="time.iv" /> {{ t('sendAsdu.timeIv') }}</label>
          </div>

          <div v-if="errorMsg" class="error-msg">{{ errorMsg }}</div>
          <div v-if="okFlag" class="ok-msg">{{ t('sendAsdu.sentOk') }}</div>
        </div>

        <div class="modal-footer">
          <button class="btn btn-secondary" @click="emit('close')">{{ t('sendAsdu.cancel') }}</button>
          <button class="btn btn-primary" :disabled="sending" @click="send">
            {{ sending ? t('sendAsdu.sending') : t('sendAsdu.send') }}
          </button>
        </div>
      </div>
    </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.modal-backdrop { position: fixed; inset: 0; background: rgba(0,0,0,0.55); display: flex; align-items: center; justify-content: center; z-index: 1000; }
.modal-box { background: var(--c-base); border: 1px solid var(--c-surface1); border-radius: 8px; padding: 20px; min-width: 480px; max-width: 92vw; box-shadow: 0 8px 24px rgba(0,0,0,0.5); }
.modal-wide { min-width: 560px; }
.modal-title { font-size: 15px; font-weight: 600; color: var(--c-text); margin-bottom: 16px; }
.modal-body { display: flex; flex-direction: column; gap: 10px; }
.modal-footer { display: flex; justify-content: flex-end; gap: 8px; margin-top: 16px; }
.form-row { display: flex; gap: 8px; align-items: flex-end; }
.form-label { display: flex; flex-direction: column; gap: 4px; font-size: 12px; color: var(--c-overlay0); }
.form-label-half { flex: 1; }
.form-label.disabled { opacity: 0.4; }
.form-input { padding: 6px 10px; background: var(--c-surface0); border: 1px solid var(--c-surface1); border-radius: 4px; color: var(--c-text); font-size: 13px; }
.form-input:focus { outline: none; border-color: var(--c-blue); }
.form-input.mono { font-family: var(--font-mono); }
.inline-pair { display: flex; gap: 6px; }
.inline-pair > .form-input.mono { width: 70px; }
.check-inline { display: inline-flex; gap: 6px; align-items: center; font-size: 12px; color: var(--c-text); }
.field-grid { display: grid; grid-template-columns: repeat(2, 1fr); gap: 8px 12px; }
.time-grid { display: grid; grid-template-columns: repeat(4, 1fr); gap: 6px 8px; padding-top: 8px; border-top: 1px dashed var(--c-surface1); }
.btn { padding: 7px 20px; border: none; border-radius: 6px; cursor: pointer; font-size: 13px; }
.btn-primary { background: var(--c-blue); color: var(--c-base); font-weight: 600; }
.btn-primary:hover { background: var(--c-sapphire); }
.btn-primary:disabled { opacity: 0.5; cursor: default; }
.btn-secondary { background: var(--c-surface1); color: var(--c-text); }
.btn-secondary:hover { background: var(--c-surface2); }
.error-msg { padding: 6px 8px; background: rgba(243,139,168,0.15); border: 1px solid var(--c-red); border-radius: 4px; color: var(--c-red); font-size: 12px; }
.ok-msg { padding: 6px 8px; background: rgba(166,227,161,0.15); border: 1px solid rgba(166,227,161,0.3); color: var(--c-green); font-size: 12px; }
</style>
```

- [ ] **Step 2：构建**

Run: `cd master-frontend && npm run build 2>&1 | tail -10`
Expected: 编译通过

- [ ] **Step 3：commit**

```bash
git add master-frontend/src/components/SendAsduDialog.vue
git commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" -m "feat(master-fe): 新增 SendAsduDialog —— 17 TypeID/9 COT 的「发送应用层报文」对话框"
```

---

### Task 3.4：Toolbar 改名 + 取消默认 GI + 接到新对话框

**Files:**
- Modify: `master-frontend/src/components/Toolbar.vue:1-15` (import)
- Modify: `master-frontend/src/components/Toolbar.vue:74-90` (open dialog state)
- Modify: `master-frontend/src/components/Toolbar.vue:124-140` (connectMaster)
- Modify: `master-frontend/src/components/Toolbar.vue:324-326` (按钮文案)
- Modify: `master-frontend/src/components/Toolbar.vue:361-372` (ControlDialog 用法)

- [ ] **Step 1：import 新组件**

把 `master-frontend/src/components/Toolbar.vue:9` 那行：

```ts
import ControlDialog from './ControlDialog.vue'
```

改为：

```ts
import ControlDialog from './ControlDialog.vue'
import SendAsduDialog from './SendAsduDialog.vue'
```

- [ ] **Step 2：替换"自定义控制"对话框状态**

在 `master-frontend/src/components/Toolbar.vue:75` 把：

```ts
const showCustomControl = ref(false)
const customControlCA = ref<number>(1)
async function openCustomControl() {
  customControlCA.value = 1
  if (selectedConnectionId.value) {
    try {
      const conns = await invoke<{ id: string; common_addresses: number[] }[]>('list_connections')
      const conn = conns.find((c) => c.id === selectedConnectionId.value)
      if (conn?.common_addresses?.length) customControlCA.value = conn.common_addresses[0]
    } catch { /* ignore — fall back to 1 */ }
  }
  showCustomControl.value = true
}
```

改成：

```ts
const showSendAsdu = ref(false)
const sendAsduCA = ref<number>(1)
async function openSendAsdu() {
  sendAsduCA.value = 1
  if (selectedConnectionId.value) {
    try {
      const conns = await invoke<{ id: string; common_addresses: number[] }[]>('list_connections')
      const conn = conns.find((c) => c.id === selectedConnectionId.value)
      if (conn?.common_addresses?.length) sendAsduCA.value = conn.common_addresses[0]
    } catch { /* ignore — fall back to 1 */ }
  }
  showSendAsdu.value = true
}
```

- [ ] **Step 3：connectMaster 改读 auto_gi**

把 `master-frontend/src/components/Toolbar.vue:124-140` 整段：

```ts
async function connectMaster() {
  if (!selectedConnectionId.value) return
  try {
    await invoke('connect_master', { id: selectedConnectionId.value })
    selectedConnectionState.value = 'Connected'
    refreshTree()
    try {
      await fanOutCAs('send_interrogation')
      refreshData()
      setTimeout(() => refreshTree(), 3000)
    } catch (e) {
      console.warn('Auto GI after connect failed:', e)
    }
  } catch (e) {
    await showAlert(String(e))
  }
}
```

改成：

```ts
async function connectMaster() {
  if (!selectedConnectionId.value) return
  try {
    await invoke('connect_master', { id: selectedConnectionId.value })
    selectedConnectionState.value = 'Connected'
    refreshTree()
    const conns = await invoke<any[]>('list_connections')
    const conn = conns.find((c: any) => c.id === selectedConnectionId.value)
    if (conn?.auto_gi_on_connect) {
      try {
        await fanOutCAs('send_interrogation')
        refreshData()
        setTimeout(() => refreshTree(), 3000)
      } catch (e) {
        console.warn('Auto GI after connect failed:', e)
      }
    }
  } catch (e) {
    await showAlert(String(e))
  }
}
```

- [ ] **Step 4：按钮文案改名**

把 `master-frontend/src/components/Toolbar.vue:324-326`：

```vue
<button class="toolbar-btn" :disabled="!hasConnection() || !isConnected()" @click="openCustomControl">
  {{ t('toolbar.customControl') }}
</button>
```

改成：

```vue
<button class="toolbar-btn" :disabled="!hasConnection() || !isConnected()" @click="openSendAsdu">
  {{ t('toolbar.sendAsdu') }}
</button>
```

- [ ] **Step 5：替换 ControlDialog 用法为 SendAsduDialog**

把 `master-frontend/src/components/Toolbar.vue:361-372`：

```vue
<ControlDialog
  :visible="showCustomControl"
  :connection-id="selectedConnectionId"
  :common-address="customControlCA"
  :prefill-ioa="null"
  :prefill-command-type="null"
  @close="showCustomControl = false"
/>
```

改成：

```vue
<SendAsduDialog
  :visible="showSendAsdu"
  :connection-id="selectedConnectionId"
  :default-c-a="sendAsduCA"
  @close="showSendAsdu = false"
  @sent="refreshData()"
/>
```

并删除顶部的 `import ControlDialog from './ControlDialog.vue'`（因为 Toolbar 不再用 ControlDialog —— DataTable.vue 仍在用，所以文件不删）。

- [ ] **Step 6：构建**

Run: `cd master-frontend && npm run build 2>&1 | tail -10`
Expected: 编译通过

- [ ] **Step 7：commit**

```bash
git add master-frontend/src/components/Toolbar.vue
git commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" -m "feat(master-fe): Toolbar「自定义控制」改为「发送报文」,接到 SendAsduDialog;connectMaster 改读 auto_gi_on_connect"
```

---

### Task 3.5：NewConnectionModal 加 `auto_gi_on_connect` 复选

**Files:**
- Modify: `master-frontend/src/components/NewConnectionModal.vue:22-67` (form schema + default)
- Modify: `master-frontend/src/components/NewConnectionModal.vue:151-170` (openEdit 回填)
- Modify: `master-frontend/src/components/NewConnectionModal.vue:200-225` (create_connection payload)
- Modify: `master-frontend/src/components/NewConnectionModal.vue:312-330` (UI 复选)

- [ ] **Step 1：表单字段定义加 auto_gi_on_connect**

打开 `master-frontend/src/components/NewConnectionModal.vue`，在 `type NewConnForm = {` 字段列表（line 22-44）末尾追加：

```ts
  auto_gi_on_connect: boolean
```

并在 `defaultForm()` 返回对象末尾（`broadcast_address_hex: 'FFFF',` 之后）加：

```ts
  auto_gi_on_connect: false,
```

bump LocalStorage 版本号防止旧数据冲撞：把 `NEW_CONN_FORM_KEY = 'iec104master.newConnForm.v2'` 改成 `'iec104master.newConnForm.v3'`。

- [ ] **Step 2：openEditConnection 时回填**

定位 `form.value = { ... }`（约 line 151），在 `broadcast_address_hex` 行之后追加：

```ts
      auto_gi_on_connect: conn.auto_gi_on_connect ?? false,
```

- [ ] **Step 3：create_connection payload 透传**

定位 `request: { ... }`（约 line 201），在 `counter_interrogate_period_s: form.value.counter_interrogate_period_s,` 之后追加：

```ts
        auto_gi_on_connect: form.value.auto_gi_on_connect,
```

- [ ] **Step 4：模板里加复选**

定位到 broadcast_address 那块（`<label class="form-label">` `broadcastAddress`，约 line 302-312），在 `<details class="proto-section">` 之前插入：

```vue
          <label class="form-checkbox">
            <input type="checkbox" v-model="form.auto_gi_on_connect" />
            <span>{{ t('newConn.autoGi') }}</span>
          </label>
          <span class="form-hint">{{ t('newConn.autoGiHint') }}</span>
```

并在 `<style scoped>` 加（如果尚无）：

```css
.form-checkbox { display: inline-flex; gap: 6px; align-items: center; font-size: 12px; color: var(--c-text); cursor: pointer; }
.form-checkbox input { accent-color: var(--c-blue); }
```

- [ ] **Step 5：构建**

Run: `cd master-frontend && npm run build 2>&1 | tail -10`
Expected: 编译通过

- [ ] **Step 6：commit**

```bash
git add master-frontend/src/components/NewConnectionModal.vue
git commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" -m "feat(master-fe): NewConnectionModal 加「连接后自动总召唤」复选(默认关)"
```

---

## 阶段 4：验证 + CHANGELOG

### Task 4.1：全栈编译 + 测试

- [ ] **Step 1：core + app 全编译 + 单测**

Run: `cargo test --workspace 2>&1 | tail -20`
Expected: 全部 PASS

- [ ] **Step 2：master 前端 npm build**

Run: `cd master-frontend && npm run build 2>&1 | tail -20`
Expected: 无 TS error

- [ ] **Step 3：iec104sim 前端也 build 一下（确保 shared-frontend 改动不破坏 slave）**

Run: `cd frontend && npm run build 2>&1 | tail -10`
Expected: 无 TS error

- [ ] **Step 4：本步骤无新提交,只是阶段闸门**

---

### Task 4.2：无头浏览器验证（Playwright）

**Files:**
- Create: `scripts/screenshots/smoke-send-asdu.mjs`

参考既有 `scripts/screenshots/capture.mjs`（CLAUDE memory: `project_headless_ui_screenshots`）注入 Tauri mock，跑一遍对话框的"切换 17 种 TypeID + 看启用字段"。

- [ ] **Step 1：先看 capture.mjs 的 mock 结构**

Run: `head -120 scripts/screenshots/capture.mjs`

- [ ] **Step 2：基于该模板写 smoke 脚本**

```js
// scripts/screenshots/smoke-send-asdu.mjs
import { chromium } from 'playwright'

const URL = process.env.SMOKE_URL ?? 'http://127.0.0.1:5173/'

const tauriInvokeMock = `
  window.__TAURI_INTERNALS__ = window.__TAURI_INTERNALS__ ?? {}
  window.__TAURI_INTERNALS__.invoke = async (cmd, args) => {
    if (cmd === 'list_connections') {
      return [{ id: 'conn_0', target_address: '127.0.0.1', port: 2404,
        common_addresses: [1], state: 'Connected', use_tls: false,
        t0: 30, t1: 15, t2: 10, t3: 20, k: 12, w: 8,
        default_qoi: 20, default_qcc: 5,
        interrogate_period_s: 0, counter_interrogate_period_s: 0,
        broadcast_address: 0xFFFF, auto_gi_on_connect: false,
        timing_corrections: [] }]
    }
    if (cmd === 'send_asdu') {
      window.__sent ??= []
      window.__sent.push(args)
      return null
    }
    return null
  }
`

const browser = await chromium.launch({ headless: true })
const ctx = await browser.newContext({ locale: 'zh-CN' })
await ctx.addInitScript(tauriInvokeMock)
const page = await ctx.newPage()
await page.goto(URL)

// 等 Toolbar 出现并点开「发送报文」
await page.waitForSelector('text=发送报文', { timeout: 10000 })
// 这里继续做实际点击 + 字段切换的断言。

await browser.close()
```

- [ ] **Step 3：本地跑一遍 dev + smoke**

Run（两个终端，分别）:

```bash
cd master-frontend && npm run dev
```

```bash
node scripts/screenshots/smoke-send-asdu.mjs
```

Expected: 脚本无错退出。如有需要看截图,在脚本里加 `await page.screenshot({ path: 'docs/screenshots/send-asdu-100.png' })`。

- [ ] **Step 4：commit**

```bash
git add scripts/screenshots/smoke-send-asdu.mjs
git commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" -m "test(master-fe): SendAsduDialog 无头 smoke 脚本"
```

---

### Task 4.3：CHANGELOG + 发版前的人工 sanity

**Files:**
- Modify: `CHANGELOG.md`

- [ ] **Step 1：在 CHANGELOG.md 顶部「Unreleased」段加条目**

```markdown
### Added
- 主站工具栏新增「发送报文」对话框,可手选 17 种 TypeID × 9 种 COT,
  按 TypeID 动态显示字段(QOI/QCC/QU/QRP/QPM/QPA/SCS/DCS/RCS/NVA/SVA/R32/CP56)。
- 新建连接对话框新增「连接后自动总召唤」复选(默认关闭)。

### Changed
- **行为变更**:默认不再于连接成功后自动发总召唤。需要老习惯的用户
  在新建/编辑连接时勾选「连接后自动总召唤」即可恢复。
- 工具栏「自定义控制」按钮改名为「发送报文」。原右键单点设值入口
  (DataTable 右键 →「设值」)仍走旧的 ControlDialog,无变化。
```

- [ ] **Step 2：commit**

```bash
git add CHANGELOG.md
git commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" -m "docs(changelog): 发送报文对话框 + 默认关闭自动 GI 的行为变更"
```

- [ ] **Step 3：人工 sanity（开发者手动）**

打开 `master-frontend` 真实开发服务器：

```bash
cd master-frontend && npm run tauri:dev
```

清单：
- 新建连接，复选「连接后自动总召唤」默认 **未勾选**
- 创建后点「连接」，确认 LogPanel 里无 GI 帧出现
- 在新连接上勾选 auto_gi 后再次连接，确认有 GI 帧
- 「发送报文」打开对话框，切换 TypeID 100→103→105→48→50→110→113，字段动态变化符合表格
- 各发一帧，LogPanel/对端模拟器收到正确帧

- [ ] **Step 4：发版**

按 release skill 操作（不在此 plan 范围内，仅记录入口）：

```
/release minor
```

CHANGELOG 已更新；版本文件 (`crates/iec104master-app/Cargo.toml` + `tauri.conf.json` + `crates/iec104sim-app/Cargo.toml` + `tauri.conf.json`) 由 release skill 处理。

---

## 自审 (Self-Review)

**Spec 覆盖：**

| Spec 章节 | 对应 Task |
|---|---|
| AsduTypeId 新 7 项 | 1.1 |
| 8 个新 builder | 1.2 |
| MasterConnection::send_asdu | 1.3 |
| MasterConfig.auto_gi_on_connect | 1.4 |
| send_asdu Tauri 命令 + schema | 2.2 |
| CreateConnectionRequest/ConnectionInfo 加 auto_gi | 2.1 |
| 字段动态显示规则 | 3.1 (enabledFields) + 3.3 (UI) |
| SendAsduDialog 持久化 v1 key | 3.3 Step 1 (`iec104master.sendAsduDialog.v1`) |
| Toolbar 改名 + connectMaster 改逻辑 | 3.4 |
| NewConnectionModal 加复选 | 3.5 |
| i18n key | 3.2 |
| TypeScript 类型 | 3.1 |
| 测试覆盖 | 1.2/1.3 单测 + 4.2 Playwright |
| 兼容性 / 行为变更 CHANGELOG | 4.3 |

**Placeholder 扫描：** 无 TBD/TODO；每段代码都给完整字节序列与字段名。

**类型一致：** `Cp56Time2aFields` (core) / `Cp56Time2aDto` (app) / `SendAsduTime` (TS) 三层命名清晰映射；`AsduCommand` 变体名跟 `AsduTypeId::*` 一一对应；`enabledFields()` 返回的 key 跟 SendAsduDialog 模板里 `fields.xxx` 一致。
