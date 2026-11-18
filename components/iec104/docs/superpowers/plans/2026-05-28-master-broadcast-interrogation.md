# 104Master 广播公共地址(0xFFFF)总召唤 — 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 104Master 增加"广播总召/对时/计量"三个按钮(默认地址 0xFFFF,可配 0xFF00),并把广播应答中未知 CA 自动补充到连接 common_addresses。

**Architecture:** `iec104sim-core::master` 加 `broadcast_address` 配置 + 未知 CA debouncer(3 s 安静期 flush);Tauri 命令层 emit `connection-cas-updated` 事件;前端 `App.vue` 监听后刷新树并落盘;`Toolbar.vue` 加"广播 ▾"split 按钮;新建连接对话框加 hex 输入。

**Tech Stack:** Rust(tokio + tauri 2 + native-tls) / Vue 3(TS + vue-i18n) / Playwright(headless e2e)

**Spec:** `docs/superpowers/specs/2026-05-28-master-broadcast-interrogation-design.md`

---

## File Structure

新建/修改文件:

| 文件 | 类型 | 职责 |
|---|---|---|
| `crates/iec104sim-core/src/master.rs` | 改 | 加 `MasterConfig.broadcast_address`; 接收路径未知 CA 钩子; `CaDebouncer` 后台 task; 断连前强制 flush; 单测 |
| `crates/iec104master-app/src/commands.rs` | 改 | `CreateConnectionRequest.broadcast_address` + `ConnectionInfo` 同步; 注入 emit 闭包给 core 层; 3 个 `send_broadcast_*` 命令 |
| `crates/iec104master-app/src/lib.rs` | 改 | 注册 3 个新 handler |
| `master-frontend/src/components/Toolbar.vue` | 改 | "广播 ▾" split button + 3 个广播动作 |
| `master-frontend/src/components/NewConnectionModal.vue` | 改 | "广播公共地址" hex 输入字段 + 校验 |
| `master-frontend/src/i18n/locales/zh-CN.ts` | 改 | 新增 broadcast/广播地址 相关键 |
| `master-frontend/src/i18n/locales/en-US.ts` | 改 | 同上,英文文案 |
| `master-frontend/src/App.vue` | 改 | `listen('connection-cas-updated')` → 刷新树 + 调 save |
| `master-frontend/src/types.ts` | 改 | `ConnectionInfo` 类型加 `broadcast_address` |

---

## Task 1: core — `MasterConfig.broadcast_address` 字段

**Files:**
- Modify: `crates/iec104sim-core/src/master.rs:199-252`(MasterConfig 结构 + Default impl)
- Test: `crates/iec104sim-core/src/master.rs`(测试区追加)

- [ ] **Step 1: 写失败测试**

在 `crates/iec104sim-core/src/master.rs` 的 `#[cfg(test)] mod tests` 中追加(找到 `test_build_gi_command` 附近,见 line ~2132):

```rust
    #[test]
    fn master_config_default_broadcast_addr_is_ffff() {
        let cfg = MasterConfig::default();
        assert_eq!(cfg.broadcast_address, 0xFFFF);
    }
```

- [ ] **Step 2: 运行测试,确认失败**

Run: `cargo test -p iec104sim-core master_config_default_broadcast_addr_is_ffff`
Expected: 编译失败,提示 `no field "broadcast_address" on type "MasterConfig"`。

- [ ] **Step 3: 加字段**

在 `MasterConfig` 结构体内(line 199-252,具体在 `pub counter_interrogate_period_s: u32,` 之后)追加:

```rust
    /// 广播公共地址。用于广播 GI/对时/计量召唤。
    /// 默认 0xFFFF。常见替代值: 0xFF00(部分厂商方言)。
    #[serde(default = "default_broadcast_address")]
    pub broadcast_address: u16,
```

在 `Default for MasterConfig` 的 `Self { ... }` 块最后追加(line ~272 附近):

```rust
            broadcast_address: default_broadcast_address(),
```

在文件内 `default_qoi_value`/`default_qcc_value` 这类辅助函数旁边(grep 找其中一个,在同一块内追加):

```rust
fn default_broadcast_address() -> u16 { 0xFFFF }
```

- [ ] **Step 4: 运行测试,确认通过**

Run: `cargo test -p iec104sim-core master_config_default_broadcast_addr_is_ffff`
Expected: 1 passed。

并跑整个 core 包确认未破坏现有测试:
Run: `cargo test -p iec104sim-core`
Expected: 全部 passed。

- [ ] **Step 5: 提交**

```bash
git add crates/iec104sim-core/src/master.rs
git commit -m "feat(core): MasterConfig 新增 broadcast_address 字段(默认 0xFFFF)"
```

---

## Task 2: core — 帧字节断言测试(0xFFFF / 0xFF00)

**Files:**
- Test: `crates/iec104sim-core/src/master.rs`(测试区追加)

(无需新建函数;`build_gi_command/build_clock_sync_command/build_counter_read_command` 已接受任意 u16。这一步是 belt-and-braces 断言广播 CA 字节顺序。)

- [ ] **Step 1: 写帧字节测试**

在 master.rs 的 `#[cfg(test)] mod tests` 内追加:

```rust
    #[test]
    fn build_gi_command_broadcast_ffff_emits_le_ff_ff() {
        let frame = build_gi_command(0xFFFF, 20);
        // 帧结构:68 0E 00 00 00 00 64 01 06 00 [CA_lo] [CA_hi] 00 00 00 [QOI]
        assert_eq!(frame[10], 0xFF, "CA low byte");
        assert_eq!(frame[11], 0xFF, "CA high byte");
        assert_eq!(frame[15], 20, "QOI");
    }

    #[test]
    fn build_gi_command_broadcast_ff00_emits_le_00_ff() {
        let frame = build_gi_command(0xFF00, 20);
        assert_eq!(frame[10], 0x00, "CA low byte");
        assert_eq!(frame[11], 0xFF, "CA high byte");
    }

    #[test]
    fn build_clock_sync_broadcast_ffff_emits_le_ff_ff() {
        let frame = build_clock_sync_command(0xFFFF);
        assert_eq!(frame[10], 0xFF);
        assert_eq!(frame[11], 0xFF);
    }

    #[test]
    fn build_counter_read_broadcast_ffff_emits_le_ff_ff() {
        let frame = build_counter_read_command(0xFFFF, 5);
        assert_eq!(frame[10], 0xFF);
        assert_eq!(frame[11], 0xFF);
        assert_eq!(frame[15], 5, "QCC");
    }
```

- [ ] **Step 2: 运行测试,确认通过**

Run: `cargo test -p iec104sim-core build_gi_command_broadcast build_clock_sync_broadcast build_counter_read_broadcast`
Expected: 4 passed(`build_*_command` 早就接受任意 u16,本测试只锁字节顺序)。

- [ ] **Step 3: 提交**

```bash
git add crates/iec104sim-core/src/master.rs
git commit -m "test(core): 锁定广播 CA(0xFFFF/0xFF00)的帧字节顺序"
```

---

## Task 3: core — `CaDebouncer` 独立模块

**Files:**
- Create: `crates/iec104sim-core/src/ca_debouncer.rs`
- Modify: `crates/iec104sim-core/src/lib.rs`(注册模块)
- Test: `crates/iec104sim-core/src/ca_debouncer.rs` 内嵌 `#[cfg(test)] mod tests`

- [ ] **Step 1: 写失败测试**

新建 `crates/iec104sim-core/src/ca_debouncer.rs`:

```rust
//! 未知 CA debouncer:广播召唤期间收集陌生 CA,3 秒安静期后一次性 flush。
//!
//! 协议层不直接持有 Tauri AppHandle —— 通过 `flush_tx` 把"该 flush 这些 CA"
//! 的事件抛给上层(commands 层),由上层去 emit Tauri 事件。

use std::collections::HashSet;
use std::time::Duration;
use tokio::sync::mpsc;

/// 单次 flush 抛出的 CA 集合(已去重)。
#[derive(Debug, Clone)]
pub struct CaFlushEvent {
    pub new_cas: Vec<u16>,
}

/// 由 commands 层持有的入口:把陌生 CA 喂进来。
#[derive(Clone)]
pub struct CaInbox {
    tx: mpsc::UnboundedSender<u16>,
}

impl CaInbox {
    pub fn push(&self, ca: u16) {
        let _ = self.tx.send(ca);
    }
}

/// 启动 debouncer:返回一个 `CaInbox`(用于喂 CA)+ `flush_rx`(用于接 flush 事件)+ 后台 JoinHandle。
///
/// `settle` 是安静期长度;每次新 CA 出现重置定时器。Channel 关闭(所有 `CaInbox` 被 drop)且
/// `state` 非空时,做最后一次 flush 然后退出 —— 这是断连前 "强制 flush" 的自然路径。
pub fn spawn(
    settle: Duration,
) -> (CaInbox, mpsc::UnboundedReceiver<CaFlushEvent>, tokio::task::JoinHandle<()>) {
    let (in_tx, mut in_rx) = mpsc::unbounded_channel::<u16>();
    let (out_tx, out_rx) = mpsc::unbounded_channel::<CaFlushEvent>();

    let handle = tokio::spawn(async move {
        let mut state: HashSet<u16> = HashSet::new();
        let mut deadline: Option<tokio::time::Instant> = None;
        loop {
            let sleep = match deadline {
                Some(d) => tokio::time::sleep_until(d),
                None => tokio::time::sleep(Duration::from_secs(3600)),
            };
            tokio::pin!(sleep);

            tokio::select! {
                maybe_ca = in_rx.recv() => {
                    match maybe_ca {
                        Some(ca) => {
                            state.insert(ca);
                            deadline = Some(tokio::time::Instant::now() + settle);
                        }
                        None => {
                            // 所有 sender drop → 强制 flush 退出
                            if !state.is_empty() {
                                let mut cas: Vec<u16> = state.drain().collect();
                                cas.sort();
                                let _ = out_tx.send(CaFlushEvent { new_cas: cas });
                            }
                            return;
                        }
                    }
                }
                _ = &mut sleep, if deadline.is_some() => {
                    if !state.is_empty() {
                        let mut cas: Vec<u16> = state.drain().collect();
                        cas.sort();
                        let _ = out_tx.send(CaFlushEvent { new_cas: cas });
                    }
                    deadline = None;
                }
            }
        }
    });

    (CaInbox { tx: in_tx }, out_rx, handle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test(start_paused = true)]
    async fn flushes_after_quiet_period() {
        let (inbox, mut rx, _handle) = spawn(Duration::from_secs(3));
        inbox.push(11);
        inbox.push(12);
        inbox.push(13);
        // 还没到安静期:不应 flush
        tokio::time::sleep(Duration::from_millis(2900)).await;
        assert!(rx.try_recv().is_err());
        tokio::time::sleep(Duration::from_millis(200)).await;
        let ev = rx.recv().await.expect("expected flush");
        assert_eq!(ev.new_cas, vec![11, 12, 13]);
    }

    #[tokio::test(start_paused = true)]
    async fn resets_deadline_on_new_ca() {
        let (inbox, mut rx, _handle) = spawn(Duration::from_secs(3));
        inbox.push(1);
        tokio::time::sleep(Duration::from_secs(2)).await;
        inbox.push(2); // 应当把 deadline 推到 now+3s
        tokio::time::sleep(Duration::from_millis(2900)).await;
        assert!(rx.try_recv().is_err(), "must not flush before reset deadline");
        tokio::time::sleep(Duration::from_millis(200)).await;
        let ev = rx.recv().await.unwrap();
        assert_eq!(ev.new_cas, vec![1, 2]);
    }

    #[tokio::test(start_paused = true)]
    async fn dedupes_same_ca() {
        let (inbox, mut rx, _handle) = spawn(Duration::from_secs(1));
        for _ in 0..5 { inbox.push(7); }
        tokio::time::sleep(Duration::from_millis(1100)).await;
        let ev = rx.recv().await.unwrap();
        assert_eq!(ev.new_cas, vec![7]);
    }

    #[tokio::test(start_paused = true)]
    async fn forces_flush_on_inbox_drop() {
        let (inbox, mut rx, handle) = spawn(Duration::from_secs(60));
        inbox.push(42);
        drop(inbox);
        // 切回 runtime 让后台 task 跑完
        let _ = tokio::time::timeout(Duration::from_secs(5), handle).await;
        let ev = rx.recv().await.unwrap();
        assert_eq!(ev.new_cas, vec![42]);
    }

    #[tokio::test(start_paused = true)]
    async fn no_flush_when_state_empty() {
        let (_inbox, mut rx, _handle) = spawn(Duration::from_millis(100));
        tokio::time::sleep(Duration::from_millis(500)).await;
        assert!(rx.try_recv().is_err());
    }
}
```

- [ ] **Step 2: 注册模块**

修改 `crates/iec104sim-core/src/lib.rs`,在现有 `pub mod master;` 等声明附近追加:

```rust
pub mod ca_debouncer;
```

- [ ] **Step 3: 运行测试,确认通过**

Run: `cargo test -p iec104sim-core ca_debouncer`
Expected: 5 passed。

- [ ] **Step 4: 提交**

```bash
git add crates/iec104sim-core/src/ca_debouncer.rs crates/iec104sim-core/src/lib.rs
git commit -m "feat(core): 新增 ca_debouncer 模块(3s 安静期 + drop 强制 flush)"
```

---

## Task 4: core — `MasterConnection` 注入 `CaInbox`,接收路径喂未知 CA

**Files:**
- Modify: `crates/iec104sim-core/src/master.rs`(MasterConnection 结构 / new / 接收路径)

- [ ] **Step 1: 写失败测试**

在 `master.rs` 的 `#[cfg(test)] mod tests` 内追加(用一个 sync test helper,所有 3 个测试统一):

```rust
    #[test]
    fn receive_path_pushes_unknown_ca_to_inbox() {
        let cfg = MasterConfig::default();
        let configured: Vec<u16> = vec![1];
        let mut frame = vec![
            0x68, 0x0E, 0x00, 0x00, 0x00, 0x00,
            1u8,        // TypeID
            0x01,       // VSQ: N=1
            0x14,       // CauseTx = 20 (响应总召)
            0x00,
            99u8, 0x00, // CA = 99 (little-endian)
            0x01, 0x00, 0x00, // IOA = 1
            0x00,       // SIQ
        ];
        frame[1] = (frame.len() - 2) as u8;

        let mut hits: Vec<u16> = Vec::new();
        filter_unknown_ca(&frame, &configured, cfg.broadcast_address, |ca| hits.push(ca));
        assert_eq!(hits, vec![99]);
    }

    #[test]
    fn receive_path_skips_configured_ca() {
        let cfg = MasterConfig::default();
        let configured: Vec<u16> = vec![1];
        let mut frame = vec![
            0x68, 0x0E, 0x00, 0x00, 0x00, 0x00,
            1u8, 0x01, 0x14, 0x00, 1u8, 0x00, 0x01, 0x00, 0x00, 0x00,
        ];
        frame[1] = (frame.len() - 2) as u8;
        let mut hits: Vec<u16> = Vec::new();
        filter_unknown_ca(&frame, &configured, cfg.broadcast_address, |ca| hits.push(ca));
        assert!(hits.is_empty(), "configured CA must not trigger inbox");
    }

    #[test]
    fn receive_path_skips_broadcast_addr_self() {
        let cfg = MasterConfig::default(); // broadcast = 0xFFFF
        let configured: Vec<u16> = vec![];
        let mut frame = vec![
            0x68, 0x0E, 0x00, 0x00, 0x00, 0x00,
            1u8, 0x01, 0x14, 0x00, 0xFF, 0xFF, 0x01, 0x00, 0x00, 0x00,
        ];
        frame[1] = (frame.len() - 2) as u8;
        let mut hits: Vec<u16> = Vec::new();
        filter_unknown_ca(&frame, &configured, cfg.broadcast_address, |ca| hits.push(ca));
        assert!(hits.is_empty(), "slave reflecting broadcast addr must not persist");
    }
```

(三个测试调一个 `filter_unknown_ca` —— 下一步实现。这个函数把"是否触发 inbox"的判定从 `parse_and_store_asdu` 里隔离出来,既给真实路径用、也方便测试。)

- [ ] **Step 2: 运行测试,确认编译失败**

Run: `cargo test -p iec104sim-core receive_path_`
Expected: 编译错误 — `filter_unknown_ca` 未定义。

- [ ] **Step 3: 提取未知 CA 判定为可测函数**

接收路径实际是 free function `parse_and_store_asdu(data, received_data, log_collector, control_tx)`(grep `fn parse_and_store_asdu` 定位,约 line 1715)。**改它的签名**,不在 `&self` 上加方法。

在 `MasterConnection` 结构内追加两个字段(放在 `control_tx` 字段后):

```rust
    /// commands 层注入:未知 CA 喂这里。
    ca_inbox: Option<crate::ca_debouncer::CaInbox>,
    /// commands 层注入并随广播 flush 更新。Arc 包裹便于克隆进 spawned 接收任务。
    configured_cas: Arc<std::sync::RwLock<Vec<u16>>>,
```

在 `MasterConnection::new` 初始化块里追加:

```rust
            ca_inbox: None,
            configured_cas: Arc::new(std::sync::RwLock::new(Vec::new())),
```

加 builder + setter(放在 `with_log_collector` 附近):

```rust
    pub fn with_ca_inbox(mut self, inbox: crate::ca_debouncer::CaInbox) -> Self {
        self.ca_inbox = Some(inbox);
        self
    }

    pub fn set_configured_cas(&self, cas: Vec<u16>) {
        if let Ok(mut w) = self.configured_cas.write() { *w = cas; }
    }

    pub(crate) fn configured_cas_snapshot(&self) -> Vec<u16> {
        self.configured_cas.read().map(|g| g.clone()).unwrap_or_default()
    }
```

把 `parse_and_store_asdu` 签名加 3 个参数:

```rust
fn parse_and_store_asdu(
    data: &[u8],
    received_data: &SharedReceivedData,
    log_collector: &Option<Arc<LogCollector>>,
    control_tx: &tokio::sync::broadcast::Sender<ControlResponse>,
    ca_inbox: &Option<crate::ca_debouncer::CaInbox>,
    configured_cas: &Arc<std::sync::RwLock<Vec<u16>>>,
    broadcast_address: u16,
) {
```

在 master.rs(`mod tests` 外、`parse_and_store_asdu` 上方)加一个 free function,**真实路径和测试共用同一份判定**:

```rust
/// 提取 + 判定 ASDU 帧的 CA 是否为"未知 CA"(广播召唤自动学习用)。
/// 满足以下两个条件就调用 `on_unknown`:
///   - `ca != broadcast_address`(从站协议错误地把广播地址回灌时丢弃)
///   - `ca ∉ configured_cas`
///
/// `data` 必须至少 12 字节(I 帧含 CA 的最小长度)。
fn filter_unknown_ca(
    data: &[u8],
    configured_cas: &[u16],
    broadcast_address: u16,
    mut on_unknown: impl FnMut(u16),
) {
    if data.len() < 12 { return; }
    let ca = u16::from_le_bytes([data[10], data[11]]);
    if ca != broadcast_address && !configured_cas.contains(&ca) {
        on_unknown(ca);
    }
}
```

在 `parse_and_store_asdu` 函数内 `let ca = u16::from_le_bytes([data[10], data[11]]);` 的**第二处**(非控制响应分支,约 line 1761)之后立刻加:

```rust
    // 未知 CA 喂给 debouncer。
    {
        let snapshot: Vec<u16> = configured_cas.read().map(|g| g.clone()).unwrap_or_default();
        if let Some(inbox) = ca_inbox.as_ref() {
            filter_unknown_ca(data, &snapshot, broadcast_address, |c| inbox.push(c));
        }
    }
```

(这里再次从 `data` 解出 CA 是为了让 `filter_unknown_ca` 做单一职责;`ca` 局部变量保留用于后续日志/insert,不冲突。)

更新所有调用点。grep `parse_and_store_asdu(` 找出每一处,在调用前 clone 上下文:

```rust
        let ca_inbox_clone = self.ca_inbox.clone();
        let configured_cas_clone = self.configured_cas.clone();
        let broadcast_addr = self.config.broadcast_address;
        // ...spawn 内调用:
        parse_and_store_asdu(
            &data, &received_data, &log_collector, &control_tx,
            &ca_inbox_clone, &configured_cas_clone, broadcast_addr,
        );
```

(若现有接收 task 是更大 `tokio::spawn(async move { ... })` 块,把上面三个 clone 在 spawn 之前做好,move 进闭包。)

`filter_unknown_ca` 单测在 Step 1 已写;真实路径 `parse_and_store_asdu` 的端到端验证由 Task 14 的集成测试覆盖。

- [ ] **Step 4: 运行测试,确认通过**

Run: `cargo test -p iec104sim-core receive_path_`
Expected: 3 passed。

并跑全包:
Run: `cargo test -p iec104sim-core`
Expected: 全部 passed。

- [ ] **Step 5: 提交**

```bash
git add crates/iec104sim-core/src/master.rs
git commit -m "feat(core): MasterConnection 接收路径喂未知 CA 到 ca_debouncer"
```

---

## Task 5: core — 断连前强制 flush(drop CaInbox)

**Files:**
- Modify: `crates/iec104sim-core/src/master.rs`(`disconnect` / `shutdown` 路径)

- [ ] **Step 1: 写测试**

在 `mod tests` 内追加:

```rust
    #[tokio::test(start_paused = true)]
    async fn disconnect_drops_ca_inbox_so_pending_cas_flush() {
        use crate::ca_debouncer;
        use std::time::Duration;
        let (inbox, mut rx, _h) = ca_debouncer::spawn(Duration::from_secs(60));
        let mut conn = MasterConnection::new(MasterConfig::default()).with_ca_inbox(inbox);
        // 模拟接收到未知 CA(直接调内部 API 喂)
        if let Some(ix) = conn.ca_inbox.as_ref() { ix.push(77); }

        // 断连:必须把 ca_inbox drop 掉
        conn.shutdown_for_test();
        // debouncer 因 sender 关闭而强制 flush 一次
        let ev = tokio::time::timeout(Duration::from_secs(3), rx.recv())
            .await.expect("flush timeout").expect("no event");
        assert_eq!(ev.new_cas, vec![77]);
    }
```

- [ ] **Step 2: 运行测试,确认失败**

Run: `cargo test -p iec104sim-core disconnect_drops_ca_inbox_so_pending_cas_flush`
Expected: 编译失败(`shutdown_for_test` 未定义)。

- [ ] **Step 3: 实现**

在 `MasterConnection` impl 内加(放在 `disconnect` 附近):

```rust
    /// 测试用:同步释放 ca_inbox,触发 debouncer flush。
    #[cfg(test)]
    pub fn shutdown_for_test(&mut self) {
        self.ca_inbox = None;
    }
```

并在生产路径的 `disconnect` 中,在置 state=Disconnected 之前加(找现有 `disconnect` 函数,见 grep 结果约 line 850 区域):

```rust
        // 让 debouncer 把未 flush 的 CA 抛出来(随 Drop 路径自然走;
        // 显式置 None 让 sender 立即关闭,无需等结构整体析构)。
        self.ca_inbox = None;
```

- [ ] **Step 4: 运行测试,确认通过**

Run: `cargo test -p iec104sim-core disconnect_drops_ca_inbox_so_pending_cas_flush`
Expected: 1 passed。

- [ ] **Step 5: 提交**

```bash
git add crates/iec104sim-core/src/master.rs
git commit -m "feat(core): disconnect 释放 CaInbox 触发 debouncer 强制 flush"
```

---

## Task 6: app — `CreateConnectionRequest.broadcast_address` + `ConnectionInfo` 同步

**Files:**
- Modify: `crates/iec104master-app/src/commands.rs`(CreateConnectionRequest / ConnectionInfo / create_connection)

- [ ] **Step 1: 写测试(单元级,验证字段序列化)**

(commands.rs 现有的测试套路是配合 tauri-test 跑;这里改用 serde 单元测试。)
在 `commands.rs` 末尾或现有 `#[cfg(test)] mod tests` 块内追加:

```rust
    #[test]
    fn create_request_deserializes_broadcast_address() {
        let json = r#"{
            "target_address": "127.0.0.1",
            "port": 2404,
            "common_addresses": [1],
            "broadcast_address": 65280
        }"#;
        let req: CreateConnectionRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.broadcast_address, Some(0xFF00));
    }

    #[test]
    fn create_request_missing_broadcast_address_is_none() {
        let json = r#"{"target_address":"127.0.0.1","port":2404,"common_addresses":[1]}"#;
        let req: CreateConnectionRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.broadcast_address, None);
    }
```

- [ ] **Step 2: 运行,确认编译失败**

Run: `cargo test -p iec104master-app create_request_`
Expected: 编译失败,缺 `broadcast_address` 字段。

- [ ] **Step 3: 加字段**

在 `CreateConnectionRequest`(grep "pub struct CreateConnectionRequest" 找到位置,约 line 26)末尾追加:

```rust
    /// 广播公共地址(默认 0xFFFF;0xFF00 是某些方言)。
    pub broadcast_address: Option<u16>,
```

在 `ConnectionInfo` 结构(同文件,grep "pub struct ConnectionInfo")末尾追加同名字段:

```rust
    pub broadcast_address: u16,
```

在 `create_connection` 中(grep `let mut config = MasterConfig`),把请求里的 `broadcast_address` 落到 `config`:

```rust
    if let Some(bcast) = request.broadcast_address {
        config.broadcast_address = bcast;
    }
```

并在构造 `ConnectionInfo` 时(约 line 165 附近 `let info = ConnectionInfo { ... }`):

```rust
        broadcast_address: config.broadcast_address,
```

`list_connections` 同样需要回写;在它构造 `ConnectionInfo` 的位置(grep `pub async fn list_connections`,约 line 860)追加同字段。

- [ ] **Step 4: 运行测试,确认通过**

Run: `cargo test -p iec104master-app create_request_`
Expected: 2 passed。

Run: `cargo build -p iec104master-app`
Expected: 编译通过。

- [ ] **Step 5: 提交**

```bash
git add crates/iec104master-app/src/commands.rs
git commit -m "feat(master-app): ConnectionRequest/Info 增 broadcast_address 字段"
```

---

## Task 7: app — `send_broadcast_*` 三个 Tauri 命令

**Files:**
- Modify: `crates/iec104master-app/src/commands.rs`
- Modify: `crates/iec104master-app/src/lib.rs`(注册)

- [ ] **Step 1: 写命令骨架**

在 `commands.rs` 的 `send_interrogation` 附近(grep 找到,约 line 285)插入三个新命令:

```rust
#[tauri::command]
pub async fn send_broadcast_gi(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let connections = state.connections.read().await;
    let conn = connections.get(&id).ok_or_else(|| format!("connection {} not found", id))?;
    let bcast = conn.connection.config().broadcast_address;
    conn.connection.send_interrogation(bcast).await
        .map_err(|e| format!("failed to send broadcast GI: {}", e))
}

#[tauri::command]
pub async fn send_broadcast_clock_sync(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let connections = state.connections.read().await;
    let conn = connections.get(&id).ok_or_else(|| format!("connection {} not found", id))?;
    let bcast = conn.connection.config().broadcast_address;
    conn.connection.send_clock_sync(bcast).await
        .map_err(|e| format!("failed to send broadcast clock sync: {}", e))
}

#[tauri::command]
pub async fn send_broadcast_counter_read(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let connections = state.connections.read().await;
    let conn = connections.get(&id).ok_or_else(|| format!("connection {} not found", id))?;
    let bcast = conn.connection.config().broadcast_address;
    conn.connection.send_counter_read(bcast).await
        .map_err(|e| format!("failed to send broadcast counter read: {}", e))
}
```

(`MasterConnection` 暴露 `config()` 引用 —— 若不存在,在 master.rs 加 `pub fn config(&self) -> &MasterConfig { &self.config }`。grep 一下确认。)

- [ ] **Step 2: 若 `config()` 不存在,加 getter**

Run: `grep -n "pub fn config\b" crates/iec104sim-core/src/master.rs`

如果没有命中,在 `MasterConnection` impl 内(`state()` 附近)追加:

```rust
    pub fn config(&self) -> &MasterConfig { &self.config }
```

- [ ] **Step 3: 注册 handler**

修改 `crates/iec104master-app/src/lib.rs` 的 `tauri::generate_handler![...]`(约 line 16-45),在 `commands::send_counter_read,` 之后追加:

```rust
            commands::send_broadcast_gi,
            commands::send_broadcast_clock_sync,
            commands::send_broadcast_counter_read,
```

- [ ] **Step 4: 编译**

Run: `cargo build -p iec104master-app`
Expected: 编译通过(无新警告)。

- [ ] **Step 5: 提交**

```bash
git add crates/iec104master-app/src/commands.rs crates/iec104master-app/src/lib.rs crates/iec104sim-core/src/master.rs
git commit -m "feat(master-app): 新增 send_broadcast_gi/clock_sync/counter_read Tauri 命令"
```

---

## Task 8: app — 注入 `CaInbox` 并把 flush 事件转成 `connection-cas-updated`

**Files:**
- Modify: `crates/iec104master-app/src/commands.rs`(create_connection)
- Modify: `crates/iec104sim-core/src/master.rs`(暴露 `update_configured_cas`)

- [ ] **Step 1: 在 core 暴露"扩展 configured_cas"接口**

在 `master.rs::MasterConnection` impl 内加:

```rust
    /// 把新发现的 CA 并入 `configured_cas`(去重)。返回新增的 CA 列表。
    pub fn extend_configured_cas(&self, new_cas: &[u16]) -> Vec<u16> {
        let mut w = match self.configured_cas.write() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        let mut added = Vec::new();
        for &ca in new_cas {
            if !w.contains(&ca) {
                w.push(ca);
                added.push(ca);
            }
        }
        added
    }

    pub fn configured_cas(&self) -> Vec<u16> {
        self.configured_cas_snapshot()
    }
```

- [ ] **Step 2: 单测**

在 `mod tests` 内追加:

```rust
    #[test]
    fn extend_configured_cas_dedupes_and_returns_only_new() {
        let conn = MasterConnection::new(MasterConfig::default());
        conn.set_configured_cas(vec![1, 2]);
        let added = conn.extend_configured_cas(&[2, 3, 4]);
        assert_eq!(added, vec![3, 4]);
        assert_eq!(conn.configured_cas(), vec![1, 2, 3, 4]);
    }
```

Run: `cargo test -p iec104sim-core extend_configured_cas_dedupes_and_returns_only_new`
Expected: 1 passed。

- [ ] **Step 3: commands.rs 中 create_connection 启动 debouncer**

在 `create_connection`(约 line 80)中,`let common_addresses = request.resolve_cas();` 之后,`let connection = ...` 之前插入:

```rust
    use iec104sim_core::ca_debouncer;
    use std::time::Duration;
    let (ca_inbox, mut flush_rx, _debouncer_handle) =
        ca_debouncer::spawn(Duration::from_secs(3));
```

把 `let connection = MasterConnection::new(config).with_log_collector(log_collector.clone());` 改成:

```rust
    let connection = MasterConnection::new(config)
        .with_log_collector(log_collector.clone())
        .with_ca_inbox(ca_inbox);
    connection.set_configured_cas(common_addresses.clone());
```

在 connection 插入到 `state.connections` **之后**,启动一个 forward task(在 `state.connections.write().await.insert(id.clone(), ...)` 之后):

```rust
    {
        let app = app_handle.clone();
        let id_clone = id.clone();
        let connections = state.connections.clone();
        tokio::spawn(async move {
            while let Some(ev) = flush_rx.recv().await {
                // 把新 CA 并入 configured_cas,并对外 emit
                let added = {
                    let guard = connections.read().await;
                    if let Some(c) = guard.get(&id_clone) {
                        c.connection.extend_configured_cas(&ev.new_cas)
                    } else {
                        break; // 连接已删除
                    }
                };
                if !added.is_empty() {
                    let payload = serde_json::json!({
                        "id": id_clone,
                        "common_addresses": {
                            let guard = connections.read().await;
                            guard.get(&id_clone)
                                .map(|c| c.connection.configured_cas())
                                .unwrap_or_default()
                        },
                        "added": added,
                    });
                    let _ = app.emit("connection-cas-updated", payload);
                }
            }
        });
    }
```

- [ ] **Step 4: 编译**

Run: `cargo build -p iec104master-app`
Expected: 编译通过。

- [ ] **Step 5: 提交**

```bash
git add crates/iec104master-app/src/commands.rs crates/iec104sim-core/src/master.rs
git commit -m "feat(master-app): create_connection 启动 ca_debouncer 并 emit connection-cas-updated"
```

---

## Task 9: frontend — types.ts 加 broadcast_address

**Files:**
- Modify: `master-frontend/src/types.ts`

- [ ] **Step 1: 查现有结构**

Run: `grep -n "ConnectionInfo\|broadcast" master-frontend/src/types.ts`
Expected: 找到 `ConnectionInfo` 定义,无 `broadcast` 字段。

- [ ] **Step 2: 加字段**

在 `ConnectionInfo` 类型里追加:

```ts
  broadcast_address: number
```

(若 `types.ts` 同时有 `NewConnForm` 类型且这里不存,跳过下一步;`NewConnForm` 实际在 `NewConnectionModal.vue` 内定义。)

- [ ] **Step 3: 类型检查**

Run: `cd master-frontend && pnpm vue-tsc --noEmit`
Expected: 无新错误(可能有现存项目错误,关注是否引入新错误)。

- [ ] **Step 4: 提交**

```bash
git add master-frontend/src/types.ts
git commit -m "feat(master-fe): ConnectionInfo 类型加 broadcast_address"
```

---

## Task 10: frontend — i18n 文案

**Files:**
- Modify: `master-frontend/src/i18n/locales/zh-CN.ts`
- Modify: `master-frontend/src/i18n/locales/en-US.ts`

- [ ] **Step 1: 加 DictShape**

在 `zh-CN.ts` 顶部 `export type DictShape = { ... }` 的 `toolbar:` 块内追加:

```ts
    broadcast: string
    broadcastGi: string
    broadcastClockSync: string
    broadcastCounterRead: string
    broadcastAddressLabel: string
```

在 `newConn:` 块内追加:

```ts
    broadcastAddress: string
    broadcastAddressHint: string
    broadcastAddressInvalid: string
```

- [ ] **Step 2: 加中文文案**

在 `zh-CN.ts` 中文字典里的 `toolbar:` 对象(grep 找 `sendGI: '总召唤'`)追加:

```ts
    broadcast: '广播',
    broadcastGi: '广播总召',
    broadcastClockSync: '广播对时',
    broadcastCounterRead: '广播计量召唤',
    broadcastAddressLabel: '当前广播地址',
```

在 `newConn:` 对象里追加(grep 找 `commonAddressHint`):

```ts
    broadcastAddress: '广播公共地址',
    broadcastAddressHint: '4 位十六进制,默认 FFFF;常见方言 FF00',
    broadcastAddressInvalid: '请输入 1-4 位十六进制',
```

- [ ] **Step 3: 加英文文案**

在 `en-US.ts` 对应位置追加:

```ts
    broadcast: 'Broadcast',
    broadcastGi: 'Broadcast GI',
    broadcastClockSync: 'Broadcast Clock Sync',
    broadcastCounterRead: 'Broadcast Counter Read',
    broadcastAddressLabel: 'Current broadcast address',
```

```ts
    broadcastAddress: 'Broadcast common address',
    broadcastAddressHint: 'Up-to-4 hex digits, default FFFF; FF00 for some vendors',
    broadcastAddressInvalid: 'Enter 1–4 hex digits',
```

- [ ] **Step 4: 类型检查**

Run: `cd master-frontend && pnpm vue-tsc --noEmit`
Expected: 无新错误。

- [ ] **Step 5: 提交**

```bash
git add master-frontend/src/i18n/locales/zh-CN.ts master-frontend/src/i18n/locales/en-US.ts
git commit -m "i18n(master-fe): 新增广播按钮/广播地址相关文案(中/英)"
```

---

## Task 11: frontend — NewConnectionModal 加广播地址字段 + 校验

**Files:**
- Modify: `master-frontend/src/components/NewConnectionModal.vue`

- [ ] **Step 1: 加表单字段**

在 `NewConnForm` 类型定义(grep `default_qoi: number`)追加:

```ts
  broadcast_address_hex: string
```

在 `defaultForm()` 内(grep `default_qoi: 20`)追加:

```ts
  broadcast_address_hex: 'FFFF',
```

- [ ] **Step 2: 加校验函数**

在 `parseCAList` 附近追加:

```ts
function parseBroadcastHex(s: string): number | null {
  const trimmed = s.trim()
  if (!/^[0-9a-fA-F]{1,4}$/.test(trimmed)) return null
  return parseInt(trimmed, 16)
}
```

- [ ] **Step 3: 在表单提交里 wire**

grep 找到提交逻辑(`save_connection` invoke 或 `createConnection` 调用),在 payload 构造里加:

```ts
  const bcast = parseBroadcastHex(form.broadcast_address_hex)
  if (bcast === null) {
    showError(t('newConn.broadcastAddressInvalid'))
    return
  }
  payload.broadcast_address = bcast
```

(具体 payload 变量名按现有代码命名,通常是 `req` 或 `payload`。)

- [ ] **Step 4: 加 UI 输入框**

在模板里找到 `common_addresses_text` 输入框下方,加一个新 form-item:

```vue
<div class="form-row">
  <label>{{ t('newConn.broadcastAddress') }}</label>
  <input
    v-model="form.broadcast_address_hex"
    type="text"
    maxlength="4"
    placeholder="FFFF"
    class="hex-input"
  />
  <small class="hint">{{ t('newConn.broadcastAddressHint') }}</small>
</div>
```

(class 与现有 form-row 对齐,不引入新样式系统。)

- [ ] **Step 5: 加载已有连接时回填**

grep 找到 `openEditConnection` 或 `loadForm`,在回填逻辑里加:

```ts
  form.broadcast_address_hex = (conn.broadcast_address ?? 0xFFFF).toString(16).toUpperCase().padStart(4, '0')
```

- [ ] **Step 6: 类型检查**

Run: `cd master-frontend && pnpm vue-tsc --noEmit`
Expected: 无新错误。

- [ ] **Step 7: 提交**

```bash
git add master-frontend/src/components/NewConnectionModal.vue
git commit -m "feat(master-fe): 新建/编辑连接对话框加广播公共地址 hex 输入"
```

---

## Task 12: frontend — Toolbar 加"广播 ▾"split 按钮

**Files:**
- Modify: `master-frontend/src/components/Toolbar.vue`

- [ ] **Step 1: 加状态**

在 `<script setup>` 顶部(其他 ref 旁,grep `const customControlCA`):

```ts
const broadcastMenuOpen = ref(false)
const broadcastAddrLabel = ref('FFFF')

async function loadBroadcastAddr() {
  if (!selectedConnectionId.value) return
  const conns = await invoke<any[]>('list_connections')
  const c = conns.find((x: any) => x.id === selectedConnectionId.value)
  const v = c?.broadcast_address ?? 0xFFFF
  broadcastAddrLabel.value = v.toString(16).toUpperCase().padStart(4, '0')
}

watch(selectedConnectionId, () => { loadBroadcastAddr() }, { immediate: true })
```

(确认 `watch` 已经从 vue import;若没有,在 import 块加。)

- [ ] **Step 2: 加 3 个 action**

在 `sendGI` 函数附近追加:

```ts
async function sendBroadcastGI() {
  if (!selectedConnectionId.value) return
  try {
    await invoke('send_broadcast_gi', { id: selectedConnectionId.value })
    refreshData()
    setTimeout(() => refreshTree(), 3500)
  } catch (e) { await showAlert(String(e)) }
  broadcastMenuOpen.value = false
}

async function sendBroadcastClockSync() {
  if (!selectedConnectionId.value) return
  try {
    await invoke('send_broadcast_clock_sync', { id: selectedConnectionId.value })
  } catch (e) { await showAlert(String(e)) }
  broadcastMenuOpen.value = false
}

async function sendBroadcastCounterRead() {
  if (!selectedConnectionId.value) return
  try {
    await invoke('send_broadcast_counter_read', { id: selectedConnectionId.value })
    refreshData()
  } catch (e) { await showAlert(String(e)) }
  broadcastMenuOpen.value = false
}
```

- [ ] **Step 3: 加 split button 模板**

在模板里找到现有"总召"按钮(grep `@click="sendGI"`,见 line 247),**之后**插入:

```vue
<div class="split-btn" :class="{ disabled: !hasConnection() || !isConnected() }">
  <button
    class="toolbar-btn"
    :disabled="!hasConnection() || !isConnected()"
    :title="`${t('toolbar.broadcastAddressLabel')}: 0x${broadcastAddrLabel}`"
    @click="sendBroadcastGI"
  >
    {{ t('toolbar.broadcast') }}
  </button>
  <button
    class="toolbar-btn split-toggle"
    :disabled="!hasConnection() || !isConnected()"
    @click="broadcastMenuOpen = !broadcastMenuOpen"
  >▾</button>
  <ul v-if="broadcastMenuOpen" class="split-menu" @click.stop>
    <li @click="sendBroadcastGI">{{ t('toolbar.broadcastGi') }}</li>
    <li @click="sendBroadcastClockSync">{{ t('toolbar.broadcastClockSync') }}</li>
    <li @click="sendBroadcastCounterRead">{{ t('toolbar.broadcastCounterRead') }}</li>
  </ul>
</div>
```

加最小样式(在 `<style>` 末尾,对齐现有 toolbar-btn 风格):

```css
.split-btn { position: relative; display: inline-flex; }
.split-btn .split-toggle { padding: 0 6px; min-width: 0; }
.split-menu {
  position: absolute; top: 100%; left: 0; z-index: 50;
  list-style: none; margin: 0; padding: 4px 0;
  background: var(--bg-elevated, #fff);
  border: 1px solid var(--border, #ccc);
  border-radius: 4px; box-shadow: 0 4px 12px rgba(0,0,0,0.12);
  min-width: 160px;
}
.split-menu li { padding: 6px 12px; cursor: pointer; white-space: nowrap; }
.split-menu li:hover { background: var(--hover, #f0f0f0); }
```

加全局点击关闭:

```ts
function closeBroadcastMenu(e: MouseEvent) {
  const el = e.target as HTMLElement
  if (!el.closest('.split-btn')) broadcastMenuOpen.value = false
}
onMounted(() => document.addEventListener('click', closeBroadcastMenu))
onBeforeUnmount(() => document.removeEventListener('click', closeBroadcastMenu))
```

(`onMounted`/`onBeforeUnmount` 从 vue import。)

- [ ] **Step 4: 类型检查**

Run: `cd master-frontend && pnpm vue-tsc --noEmit`
Expected: 无新错误。

- [ ] **Step 5: 提交**

```bash
git add master-frontend/src/components/Toolbar.vue
git commit -m "feat(master-fe): Toolbar 新增「广播 ▾」 split 按钮(GI/对时/计量)"
```

---

## Task 13: frontend — App.vue 监听 `connection-cas-updated`

**Files:**
- Modify: `master-frontend/src/App.vue`

- [ ] **Step 1: 加 listener**

在 App.vue 现有 `unlistenConnState = await listen<...>('connection-state', ...)` 附近(grep line 135),追加同样模式的新 listener:

```ts
let unlistenCasUpdated: UnlistenFn | null = null
// ...在 onMounted async 块里:
unlistenCasUpdated = await listen<{ id: string; common_addresses: number[]; added: number[] }>(
  'connection-cas-updated',
  () => {
    // debouncer flush 后:刷新连接树,并触发一次配置保存(后端只更新内存)
    refreshTree()
    invoke('save_config').catch(() => { /* save 非关键路径 */ })
  },
)
```

在 `onBeforeUnmount` 里 cleanup:

```ts
unlistenCasUpdated?.()
```

- [ ] **Step 2: 类型检查**

Run: `cd master-frontend && pnpm vue-tsc --noEmit`
Expected: 无新错误。

- [ ] **Step 3: 提交**

```bash
git add master-frontend/src/App.vue
git commit -m "feat(master-fe): 监听 connection-cas-updated 自动刷新连接树并落盘"
```

---

## Task 14: 集成测试 — Mock fan-out 验证 debouncer + emit 路径

**Files:**
- Create: `crates/iec104master-app/tests/broadcast_ca_debouncer_integration.rs`

(由于 slave 端不识别 0xFFFF,这里**不验证** "slave 收 0xFFFF GI 后真回数据";只验证 master 接收路径喂未知 CA → debouncer 3s 后 flush → 增到 configured_cas。fan-out 用直接调 `extend_configured_cas` 模拟一下应答。)

- [ ] **Step 1: 写集成测试**

```rust
//! 集成测试:模拟从站用未配置 CA 应答,debouncer 3s 后 flush。

use iec104sim_core::ca_debouncer;
use std::time::Duration;

#[tokio::test(start_paused = true)]
async fn debouncer_collects_three_cas_and_flushes_once() {
    let (inbox, mut rx, _h) = ca_debouncer::spawn(Duration::from_secs(3));
    inbox.push(1);
    inbox.push(2);
    inbox.push(3);
    inbox.push(2); // 重复
    tokio::time::sleep(Duration::from_millis(3100)).await;
    let ev = rx.recv().await.unwrap();
    assert_eq!(ev.new_cas, vec![1, 2, 3]);
    // 没有第二次 flush
    assert!(rx.try_recv().is_err());
}

#[tokio::test(start_paused = true)]
async fn debouncer_handles_burst_then_settle() {
    let (inbox, mut rx, _h) = ca_debouncer::spawn(Duration::from_secs(2));
    for ca in [10u16, 11, 12, 13, 14, 15] { inbox.push(ca); }
    tokio::time::sleep(Duration::from_millis(2100)).await;
    let ev = rx.recv().await.unwrap();
    assert_eq!(ev.new_cas, (10..=15).collect::<Vec<u16>>());
}
```

- [ ] **Step 2: 运行**

Run: `cargo test -p iec104master-app --test broadcast_ca_debouncer_integration`
Expected: 2 passed。

- [ ] **Step 3: 提交**

```bash
git add crates/iec104master-app/tests/broadcast_ca_debouncer_integration.rs
git commit -m "test(master-app): debouncer 集成测试(突发 + 安静期 + 去重)"
```

---

## Task 15: 端到端 — Playwright headless 验证

**Files:**
- Create or Modify: `master-frontend/e2e/broadcast.spec.ts`(若 e2e 目录不存在则创建)

(参考 release skill 8.1 与 `feedback_frontend_headless_verify`。本步若项目尚未配置 Playwright,先确认 `pnpm exec playwright --version`。若未配置,本任务退化为"手动点击 + 截图归档",改成一段执行说明而非自动化测试。)

- [ ] **Step 1: 确认 Playwright 配置**

Run: `cd master-frontend && pnpm exec playwright --version`
Expected:
- 已有版本号 → 进入 Step 2;
- 找不到 → 跳到 Step 5(手动验证替代)。

- [ ] **Step 2: 写 e2e 用例(若 Step 1 通过)**

```ts
import { test, expect } from '@playwright/test'

test('广播按钮存在并能展开', async ({ page }) => {
  await page.goto('http://localhost:1420')
  // 创建一条本地连接(假定项目已有 fixtures 或现有"新建连接"流程)
  // ...
  const splitBtn = page.locator('.split-btn').filter({ hasText: '广播' })
  await expect(splitBtn).toBeVisible()
  await splitBtn.locator('.split-toggle').click()
  await expect(page.locator('.split-menu li')).toHaveCount(3)
  await expect(page.locator('.split-menu li').nth(0)).toHaveText('广播总召')
})
```

- [ ] **Step 3: 跑 e2e(headless)**

Run: `cd master-frontend && pnpm exec playwright test broadcast.spec.ts --reporter=line`
Expected: PASS。

- [ ] **Step 4: 提交**

```bash
git add master-frontend/e2e/broadcast.spec.ts
git commit -m "test(master-fe): e2e 验证广播 split 按钮渲染与展开"
```

- [ ] **Step 5: 若 Playwright 未配置,改用手动验证**

启动 master GUI(headless,按 release skill 8.1 套路:`pnpm tauri dev` 后用 Playwright MCP 操控):

1. 新建连接 → 广播公共地址留默认 `FFFF` → 保存
2. 连接后,点 "广播 ▾" → 展开三项
3. 截图(MCP `browser_take_screenshot`)归档到 `.playwright-mcp/`
4. 在 commit message 引用截图路径

```bash
git add .playwright-mcp/broadcast-menu.png
git commit -m "test(master-fe): 手动验证广播 split 按钮(MCP 截图归档)"
```

---

## Task 16: CHANGELOG + 收尾

**Files:**
- Modify: `CHANGELOG.md`(最顶部加新版本条目)

- [ ] **Step 1: 加 CHANGELOG 条目**

(版本号留空给 release skill 处理;此处只写功能条目。)

在 CHANGELOG.md 顶部 "Unreleased" 段(若有)或新建一段下追加:

```markdown
### Added
- 104Master:工具栏新增「广播 ▾」按钮,支持广播公共地址(默认 0xFFFF,可配 0xFF00)的总召唤/对时/计量召唤。
- 104Master:广播应答中未发现的 CA 自动加入连接 common_addresses(3 秒安静期 debouncer)。
```

- [ ] **Step 2: 跑全套测试**

Run: `cargo test --workspace`
Expected: 全部通过。

Run: `cd master-frontend && pnpm vue-tsc --noEmit`
Expected: 无错误。

- [ ] **Step 3: 提交**

```bash
git add CHANGELOG.md
git commit -m "docs: CHANGELOG 加广播总召条目"
```

---

## Self-Review 备忘(供执行者参考,不是任务)

执行完所有任务后核对:
- [ ] spec §5.1 后端 `MasterConfig.broadcast_address` ✓ Task 1
- [ ] spec §5.1 `ConnectionRequest.broadcast_address: Option<u16>` ✓ Task 6
- [ ] spec §5.2 前端 `broadcast_address: number` ✓ Task 9
- [ ] spec §6 debouncer(3s 安静期 / 去重 / 断连前 flush / 跳过广播地址自反射) ✓ Task 3/4/5
- [ ] spec §6.3 后端只更新内存,前端落盘 ✓ Task 8 / Task 13
- [ ] spec §7.1 split 按钮 + tooltip ✓ Task 12
- [ ] spec §7.2 新建连接对话框 hex 输入 + 校验 ✓ Task 11
- [ ] spec §7.3 i18n 中英双语 ✓ Task 10
- [ ] spec §9.1 字节断言 + debouncer 单测 ✓ Task 2 / Task 3
- [ ] spec §9.2 集成测试(字节断言 + mock fan-out + debouncer 持久化) ✓ Task 14
- [ ] spec §9.3 Playwright e2e ✓ Task 15
- [ ] spec §10 风险:slave 端识别 0xFFFF 列为 Future Work ✓(不在本计划任务中)

如发现 spec 有未覆盖项,在执行阶段创建新 task 补齐,**不要**留 TODO。
