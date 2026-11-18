# 周期变位下沉到点位 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把子站「固定变位」从手填 IOA/类型/周期的独立面板，改为在点表上右键直接对所选点位启停周期变位，并支持多点并发。

**Architecture:** 后端 `SlaveServer` 从单个固定变位任务句柄改为按 `(ca, ioa, asdu_type)` 维护的多任务 `HashMap`，新增 `start_point_mutation` / `stop_point_mutation` / `list_point_mutations` 三个方法与对应 Tauri 命令；删除 `FixedMutationConfig` 与 `set_fixed_mutation`。前端在 `DataPointTable.vue` 右键菜单加入「启动/停止周期变位」+ 周期输入与脉冲指示，移除 `RemoteParamsForm/Drawer/Modal` 里的固定变位 UI。类型经 `parse_asdu_type` 解析，点位自带 IOA/类型，从根上消除原 `m_me_nc_1` 不匹配 bug。

**Tech Stack:** Rust (tokio / Tauri v2) · Vue 3 (`<script setup>` + Composition API) · Vitest 风格无 / cargo test · Playwright 无头验证

---

## File Structure

**后端（Rust）**
- `crates/iec104sim-core/src/slave.rs` — 删 `FixedMutationConfig`、`fixed_mutation_handle`、`RemoteOperationConfig.fixed_mutation`、`set_fixed_mutation`；加 `point_mutation_handles` 字段与 `start_point_mutation` / `stop_point_mutation` / `list_point_mutations`；`stop()` 清理句柄。
- `crates/iec104sim-core/tests/headless_mutation_pacing.rs` — 重写 3 个固定变位测试为新 API。
- `crates/iec104sim-app/src/commands.rs` — 删 `set_fixed_mutation` / `FixedMutationRequest` / `FixedMutationConfig` import；加 3 个新命令 + `PointMutationInfo`。复用现有 `parse_asdu_type`。
- `crates/iec104sim-app/src/lib.rs` — invoke_handler 注册替换。

**前端（Vue）**
- `frontend/src/types.ts` — 删 `FixedMutationConfig` / `fixed_mutation`；加 `PointMutationInfo`。
- `frontend/src/composables/useRemoteParams.ts` — 删 `setFixedMutation`。
- `frontend/src/components/RemoteParamsForm.vue` — 删「固定变位」分组、`asduTypeOptions`、`actions-fixed` 槽位。
- `frontend/src/components/RemoteParamsDrawer.vue` — 删 `actions-fixed` 模板、`startFixed`/`stopFixed`、dirty 基线特例及无用 CSS。
- `frontend/src/components/RemoteParamsModal.vue` — 删 `actions-fixed` 模板与无用 CSS。
- `frontend/src/components/DataPointTable.vue` — 右键菜单加启停 + 周期输入 + 活跃集追踪 + 脉冲指示。
- `frontend/src/i18n/locales/zh-CN.ts` / `en-US.ts` — `table.startMutation` / `stopMutation` / `mutationPeriod`。

---

## Task 1: 核心层 per-point 变位 API + 删除 FixedMutationConfig（TDD）

**Files:**
- Modify: `crates/iec104sim-core/src/slave.rs`
- Test: `crates/iec104sim-core/tests/headless_mutation_pacing.rs`

> 核心 crate 可独立编译测试（`cargo test -p iec104sim-core`），不依赖 app crate，因此先在此完成 TDD。

- [ ] **Step 1: 重写测试文件为新 API（先让其无法编译/失败）**

把 `crates/iec104sim-core/tests/headless_mutation_pacing.rs` 整体替换为：

```rust
//! 验证点位周期变位的起停、多点并发独立性,以及句柄登记 (list_point_mutations)。
//!
//! 注:周期变位的 Tauri 命令在 app crate;core 层暴露
//! `start_point_mutation` / `stop_point_mutation` / `list_point_mutations`。

mod common;
use common::harness::Pair;
use common::helpers::{count_iframes, wait_for_ioa_count, DEFAULT_TIMEOUT};

use iec104sim_core::data_point::DataPointValue;
use iec104sim_core::log_entry::Direction;
use iec104sim_core::slave::RemoteOperationConfig;
use iec104sim_core::types::AsduTypeId;
use tokio::time::{sleep, Duration};

/// 单点周期变位:启动后 1 秒内 master 应收到至少 3 帧 M_SP_NA_1 自发帧;
/// 停止后不再新增,且 list_point_mutations 清空。
#[tokio::test]
async fn point_mutation_starts_and_stops() {
    let pair = Pair::spawn(RemoteOperationConfig::default()).await;

    pair.master.conn.send_interrogation(1).await.unwrap();
    let _ = wait_for_ioa_count(&pair.master.conn, 1, 1, DEFAULT_TIMEOUT).await;
    pair.log.clear().await;

    pair.slave.server
        .start_point_mutation(1, 1, AsduTypeId::MSpNa1, 200)
        .await;
    assert_eq!(pair.slave.server.list_point_mutations().await.len(), 1);

    sleep(Duration::from_secs(1)).await;
    let count_during = count_iframes(&pair.log, Direction::Rx, "M_SP_NA_1").await;
    assert!(count_during >= 3, "1 秒应至少 3 帧 M_SP_NA_1,实际 {}", count_during);

    pair.slave.server.stop_point_mutation(1, 1, AsduTypeId::MSpNa1).await;
    assert!(pair.slave.server.list_point_mutations().await.is_empty());

    sleep(Duration::from_millis(300)).await;
    let baseline = count_iframes(&pair.log, Direction::Rx, "M_SP_NA_1").await;
    sleep(Duration::from_millis(500)).await;
    let after_stop = count_iframes(&pair.log, Direction::Rx, "M_SP_NA_1").await;
    assert_eq!(baseline, after_stop, "停止后不应再增加 M_SP_NA_1 帧");

    pair.shutdown().await;
}

/// 多点并发:IOA=1 与 IOA=2 同时变位,各自独立产生帧;停 IOA=1 后,
/// IOA=2 继续而 IOA=1 停止增长。
#[tokio::test]
async fn multi_point_mutation_independent() {
    let pair = Pair::spawn(RemoteOperationConfig::default()).await;
    pair.master.conn.send_interrogation(1).await.unwrap();
    let _ = wait_for_ioa_count(&pair.master.conn, 1, 2, DEFAULT_TIMEOUT).await;

    pair.slave.server.start_point_mutation(1, 1, AsduTypeId::MSpNa1, 150).await;
    pair.slave.server.start_point_mutation(1, 2, AsduTypeId::MSpNa1, 150).await;
    assert_eq!(pair.slave.server.list_point_mutations().await.len(), 2);

    let count_ioa = |frames: &Vec<iec104sim_core::log_entry::LogEntry>, ioa: &str| {
        frames.iter().filter(|e| {
            matches!(&e.frame_label, iec104sim_core::log_entry::FrameLabel::IFrame(s) if s.contains("M_SP_NA_1"))
                && e.detail.contains(ioa)
        }).count()
    };

    sleep(Duration::from_millis(600)).await;
    let frames = pair.log.get_all().await;
    assert!(count_ioa(&frames, "IOA=1") >= 2, "IOA=1 应已多次变位");
    assert!(count_ioa(&frames, "IOA=2") >= 2, "IOA=2 应已多次变位");

    // 停 IOA=1,保留 IOA=2。
    pair.slave.server.stop_point_mutation(1, 1, AsduTypeId::MSpNa1).await;
    let active = pair.slave.server.list_point_mutations().await;
    assert_eq!(active, vec![(1u16, 2u32, AsduTypeId::MSpNa1)]);

    pair.log.clear().await;
    sleep(Duration::from_millis(600)).await;
    let frames2 = pair.log.get_all().await;
    assert_eq!(count_ioa(&frames2, "IOA=1"), 0, "停止后 IOA=1 不应再变位");
    assert!(count_ioa(&frames2, "IOA=2") >= 2, "IOA=2 应继续变位");

    pair.slave.server.stop_point_mutation(1, 2, AsduTypeId::MSpNa1).await;
    pair.shutdown().await;
}

/// 翻转值确实变化:SP 点首次 tick 后从 false↔true 切换。
#[tokio::test]
async fn point_mutation_actually_flips_value() {
    let pair = Pair::spawn(RemoteOperationConfig::default()).await;
    pair.master.conn.send_interrogation(1).await.unwrap();
    let _ = wait_for_ioa_count(&pair.master.conn, 1, 1, DEFAULT_TIMEOUT).await;

    let init_b = {
        let stations = pair.slave.server.stations.read().await;
        match stations.get(&1).unwrap().data_points.get(1, AsduTypeId::MSpNa1).unwrap().value {
            DataPointValue::SinglePoint { value } => value,
            _ => panic!("默认应是 SinglePoint"),
        }
    };

    pair.slave.server.start_point_mutation(1, 1, AsduTypeId::MSpNa1, 150).await;
    sleep(Duration::from_millis(200)).await;
    let after_one = {
        let stations = pair.slave.server.stations.read().await;
        match stations.get(&1).unwrap().data_points.get(1, AsduTypeId::MSpNa1).unwrap().value {
            DataPointValue::SinglePoint { value } => value,
            _ => panic!(),
        }
    };
    assert_ne!(init_b, after_one, "首次 tick 后值应已翻转");

    pair.slave.server.stop_point_mutation(1, 1, AsduTypeId::MSpNa1).await;
    pair.shutdown().await;
}
```

- [ ] **Step 2: 运行测试确认失败（编译错误：方法/类型不存在）**

Run: `cargo test -p iec104sim-core --test headless_mutation_pacing 2>&1 | tail -20`
Expected: 编译失败，提示 `start_point_mutation` / `stop_point_mutation` / `list_point_mutations` 不存在、`FixedMutationConfig` 仍被旧代码引用。

- [ ] **Step 3: slave.rs — 删除 `FixedMutationConfig` 结构**

删除当前 132–142 行整块：

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct FixedMutationConfig {
    pub enabled: bool,
    pub ioa: u32,
    pub asdu_type: AsduTypeId,
    pub period_ms: u32,
}

impl Default for FixedMutationConfig {
    fn default() -> Self { Self { enabled: false, ioa: 1, asdu_type: AsduTypeId::MSpNa1, period_ms: 1000 } }
}
```

- [ ] **Step 4: slave.rs — 从 `RemoteOperationConfig` 删除 `fixed_mutation` 字段与默认值**

删除字段（当前第 206 行）：

```rust
    pub fixed_mutation: FixedMutationConfig,
```

删除 `Default` 里（当前第 224 行）：

```rust
            fixed_mutation: FixedMutationConfig::default(),
```

- [ ] **Step 5: slave.rs — 替换结构体字段 `fixed_mutation_handle` 为 `point_mutation_handles`**

把当前 543–545 行：

```rust
    /// 固定变位后台任务句柄。`set_fixed_mutation` 在 enabled 切换时 abort 旧任务。
    #[allow(dead_code)]
    fixed_mutation_handle: tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
```

替换为：

```rust
    /// 按 (ca, ioa, asdu_type) 维护的周期变位任务句柄。每个点位独立启停;
    /// `start_point_mutation` 对同一 key 重复调用会先 abort 旧任务。
    point_mutation_handles:
        tokio::sync::Mutex<HashMap<(u16, u32, AsduTypeId), tokio::task::JoinHandle<()>>>,
```

- [ ] **Step 6: slave.rs — `new()` 初始化替换**

把当前第 561 行：

```rust
            fixed_mutation_handle: tokio::sync::Mutex::new(None),
```

替换为：

```rust
            point_mutation_handles: tokio::sync::Mutex::new(HashMap::new()),
```

- [ ] **Step 7: slave.rs — 用三个新方法替换 `set_fixed_mutation`**

把当前 624–664 行整段 `set_fixed_mutation` 方法替换为：

```rust
    /// 启动单个点位的周期变位。同 (ca, ioa, asdu_type) 已有任务则先 abort 再起新的。
    /// period_ms 下限 50ms。任务周期性 flip_value 该点并上送 spontaneous。
    pub async fn start_point_mutation(
        &self,
        ca: u16,
        ioa: u32,
        asdu_type: AsduTypeId,
        period_ms: u32,
    ) {
        let key = (ca, ioa, asdu_type);
        let mut guard = self.point_mutation_handles.lock().await;
        if let Some(h) = guard.remove(&key) { h.abort(); }

        let stations = self.stations.clone();
        let connections = self.connections.clone();
        let remote_ops = self.remote_ops.clone();
        let log_collector = self.log_collector.clone();
        let shutdown_flag = self.shutdown_flag.clone();
        let handle = tokio::spawn(async move {
            let period = std::time::Duration::from_millis(period_ms.max(50) as u64);
            let mut interval = tokio::time::interval(period);
            interval.tick().await; // 跳过 immediate first tick
            loop {
                interval.tick().await;
                if shutdown_flag.load(std::sync::atomic::Ordering::SeqCst) { break; }
                let flipped = {
                    let mut st_guard = stations.write().await;
                    if let Some(station) = st_guard.get_mut(&ca) {
                        if let Some(p) = station.data_points.get_mut(ioa, asdu_type) {
                            p.value = flip_value(&p.value);
                            p.timestamp = Some(chrono::Utc::now());
                            // p 的可变借用到此结束(NLL),mark_changed 可重新借 data_points。
                            station.data_points.mark_changed(ioa, asdu_type);
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                };
                if flipped {
                    do_queue_spontaneous(
                        &stations, &connections, &remote_ops, &log_collector,
                        ca, &[(ioa, asdu_type)],
                    ).await;
                }
            }
        });
        guard.insert(key, handle);
    }

    /// 停止单个点位的周期变位。
    pub async fn stop_point_mutation(&self, ca: u16, ioa: u32, asdu_type: AsduTypeId) {
        let mut guard = self.point_mutation_handles.lock().await;
        if let Some(h) = guard.remove(&(ca, ioa, asdu_type)) { h.abort(); }
    }

    /// 返回当前活跃的周期变位点位 (ca, ioa, asdu_type)。
    pub async fn list_point_mutations(&self) -> Vec<(u16, u32, AsduTypeId)> {
        self.point_mutation_handles.lock().await.keys().copied().collect()
    }
```

> 说明:与原 `set_fixed_mutation` 一致,单次 `write().await` 内先 `get_mut` 翻转、再 `mark_changed`——`p` 的可变借用在 `p.timestamp = …` 后即结束(NLL),故 `mark_changed` 能重新借用 `data_points`。语义从「遍历所有 station」收窄为「仅目标 ca」。

- [ ] **Step 8: slave.rs — `stop()` 中清理全部周期变位句柄**

在 `stop()` 方法里,当前第 952 行 `if let Some(h) = self.cyclic_handle.take() { let _ = h.await; }` 之后插入：

```rust
        {
            let mut handles = self.point_mutation_handles.lock().await;
            for (_k, h) in handles.drain() { h.abort(); }
        }
```

- [ ] **Step 9: 运行测试确认通过**

Run: `cargo test -p iec104sim-core --test headless_mutation_pacing 2>&1 | tail -20`
Expected: `point_mutation_starts_and_stops`、`multi_point_mutation_independent`、`point_mutation_actually_flips_value` 三个全部 PASS。

- [ ] **Step 10: 跑核心层全量测试确保未回归**

Run: `cargo test -p iec104sim-core 2>&1 | tail -20`
Expected: 全部 PASS（无对 `FixedMutationConfig` 的悬挂引用）。

- [ ] **Step 11: Commit**

```bash
git add crates/iec104sim-core/src/slave.rs crates/iec104sim-core/tests/headless_mutation_pacing.rs
git -c user.name="Karl-Dai Karl" -c user.email="kelsoprotein@gmail.com" \
  commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" \
  -m "feat(slave-core): 周期变位改为按点位多任务并发, 移除 FixedMutationConfig"
```

---

## Task 2: App 层 Tauri 命令替换

**Files:**
- Modify: `crates/iec104sim-app/src/commands.rs`
- Modify: `crates/iec104sim-app/src/lib.rs`

- [ ] **Step 1: commands.rs — 移除 `FixedMutationConfig` import**

当前 5–8 行的 use 块：

```rust
use iec104sim_core::slave::{
    FixedMutationConfig, ProtocolTimingConfig, RemoteOperationConfig, SlaveServer,
    SlaveTransportConfig, Station,
};
```

改为（删 `FixedMutationConfig,`）：

```rust
use iec104sim_core::slave::{
    ProtocolTimingConfig, RemoteOperationConfig, SlaveServer,
    SlaveTransportConfig, Station,
};
```

- [ ] **Step 2: commands.rs — 删除 `FixedMutationRequest` 与 `set_fixed_mutation`，替换为三个新命令**

删除当前 1055–1073 行整段：

```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct FixedMutationRequest {
    pub server_id: String,
    pub config: FixedMutationConfig,
}

#[tauri::command]
pub async fn set_fixed_mutation(
    state: State<'_, AppState>,
    request: FixedMutationRequest,
) -> Result<(), String> {
    let servers = state.servers.read().await;
    let srv = servers
        .get(&request.server_id)
        .ok_or_else(|| format!("server {} not found", request.server_id))?;
    srv.server.set_fixed_mutation(request.config).await;
    Ok(())
}
```

替换为：

```rust
#[tauri::command]
pub async fn start_point_mutation(
    state: State<'_, AppState>,
    server_id: String,
    common_address: u16,
    ioa: u32,
    asdu_type: String,
    period_ms: u32,
) -> Result<(), String> {
    let asdu = parse_asdu_type(&asdu_type)?;
    let servers = state.servers.read().await;
    let srv = servers
        .get(&server_id)
        .ok_or_else(|| format!("server {} not found", server_id))?;
    srv.server
        .start_point_mutation(common_address, ioa, asdu, period_ms)
        .await;
    Ok(())
}

#[tauri::command]
pub async fn stop_point_mutation(
    state: State<'_, AppState>,
    server_id: String,
    common_address: u16,
    ioa: u32,
    asdu_type: String,
) -> Result<(), String> {
    let asdu = parse_asdu_type(&asdu_type)?;
    let servers = state.servers.read().await;
    let srv = servers
        .get(&server_id)
        .ok_or_else(|| format!("server {} not found", server_id))?;
    srv.server
        .stop_point_mutation(common_address, ioa, asdu)
        .await;
    Ok(())
}

/// list_point_mutations 返回项。asdu_type 用 .name() 大写显示名,
/// 与 list_data_points 的 DataPointInfo.asdu_type 一致,前端可直接拼 key。
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct PointMutationInfo {
    pub ioa: u32,
    pub asdu_type: String,
}

#[tauri::command]
pub async fn list_point_mutations(
    state: State<'_, AppState>,
    server_id: String,
    common_address: u16,
) -> Result<Vec<PointMutationInfo>, String> {
    let servers = state.servers.read().await;
    let srv = servers
        .get(&server_id)
        .ok_or_else(|| format!("server {} not found", server_id))?;
    let active = srv.server.list_point_mutations().await;
    Ok(active
        .into_iter()
        .filter(|(ca, _, _)| *ca == common_address)
        .map(|(_, ioa, t)| PointMutationInfo { ioa, asdu_type: t.name().to_string() })
        .collect())
}
```

- [ ] **Step 3: lib.rs — 替换 invoke_handler 注册**

当前第 50 行：

```rust
            commands::set_fixed_mutation,
```

替换为：

```rust
            commands::start_point_mutation,
            commands::stop_point_mutation,
            commands::list_point_mutations,
```

- [ ] **Step 4: 编译 app crate 确认通过**

Run: `cargo check -p iec104sim-app 2>&1 | tail -20`
Expected: 编译通过，无 `FixedMutationConfig` / `set_fixed_mutation` 残留引用错误。（`frontend/dist` 已存在，tauri-build 不报缺资源。）

- [ ] **Step 5: Commit**

```bash
git add crates/iec104sim-app/src/commands.rs crates/iec104sim-app/src/lib.rs
git -c user.name="Karl-Dai Karl" -c user.email="kelsoprotein@gmail.com" \
  commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" \
  -m "feat(slave-app): 新增 start/stop/list_point_mutation 命令, 移除 set_fixed_mutation"
```

---

## Task 3: 前端移除旧固定变位 UI

**Files:**
- Modify: `frontend/src/types.ts`
- Modify: `frontend/src/composables/useRemoteParams.ts`
- Modify: `frontend/src/components/RemoteParamsForm.vue`
- Modify: `frontend/src/components/RemoteParamsDrawer.vue`
- Modify: `frontend/src/components/RemoteParamsModal.vue`

> 本任务一并删除所有 `fixed_mutation` 引用，使前端重新可编译。`PointMutationInfo` 类型在 Task 5 加入（其使用方仅 DataPointTable）。

- [ ] **Step 1: types.ts — 删除 `FixedMutationConfig` 接口**

删除当前 95–101 行：

```typescript
export interface FixedMutationConfig {
  enabled: boolean
  ioa: number
  /** snake_case ASDU type identifier matching Rust serde enum, e.g. "m_sp_na_1". */
  asdu_type: string
  period_ms: number
}
```

- [ ] **Step 2: types.ts — 从 `RemoteOperationConfig` 删除 `fixed_mutation`**

删除当前第 128 行 `  fixed_mutation: FixedMutationConfig`，以及 `DEFAULT_REMOTE_OPS` 中当前第 148 行：

```typescript
  fixed_mutation: { enabled: false, ioa: 1, asdu_type: 'm_sp_na_1', period_ms: 1000 },
```

- [ ] **Step 3: useRemoteParams.ts — 删除 `setFixedMutation` 与相关 import/export**

- 删 import 中（当前第 6 行）`  type FixedMutationConfig,`。
- 删 `setFixedMutation` 函数整段（当前 64–73 行）。
- return 语句（当前第 77 行）去掉 `setFixedMutation`：

```typescript
  return { timing, ops, loading, lastError, load, applyTiming, applyOps }
```

- [ ] **Step 4: RemoteParamsForm.vue — 删除固定变位 UI 与 `asduTypeOptions`**

- 删 `asduTypeOptions` 常量（当前 43–46 行）。
- 删模板中「变位仿真」section 里的「固定变位」分组（当前 217–239 行整块 `<div class="rp-group">…</div>`，含 `actions-fixed` slot），保留同 section 的「随机变位节流」分组。

删除的模板块：

```vue
    <div class="rp-group">
      <span class="rp-group-label">固定变位</span>
      <div class="rp-fixed">
        <div class="rp-field">
          <label>IOA</label>
          <input type="number" min="0" max="16777215" v-model.number="ops.fixed_mutation.ioa" />
        </div>
        <div class="rp-field">
          <label>类型</label>
          <select v-model="ops.fixed_mutation.asdu_type">
            <option v-for="t in asduTypeOptions" :key="t" :value="t">{{ t }}</option>
          </select>
        </div>
        <div class="rp-field">
          <label>周期</label>
          <div class="rp-inline">
            <input type="number" min="50" max="60000" v-model.number="ops.fixed_mutation.period_ms" />
            <span class="rp-unit">ms</span>
          </div>
        </div>
      </div>
      <slot name="actions-fixed" :enabled="ops.fixed_mutation.enabled" />
    </div>
```

- 把该 section 的副标题（当前第 194 行）`<span class="rp-sec-sub">随机变位 · 固定变位</span>` 改为 `<span class="rp-sec-sub">随机变位节流</span>`。
- `.rp-fixed` 相关 CSS（当前 468–474 行的 `.rp-pacing, .rp-fixed { … }` 与 `.rp-fixed { grid-template-columns: 1fr; }`）：把选择器收敛为仅 `.rp-pacing`：

```css
.rp-pacing {
  display: grid;
  gap: 6px;
}
```

- [ ] **Step 5: RemoteParamsDrawer.vue — 删除固定变位接线与无用 CSS**

- 解构（当前第 16 行）去掉 `setFixedMutation`：

```typescript
const { timing, ops, loading, lastError, load, applyTiming, applyOps } =
  useRemoteParams(selectedServerId)
```

- `snapshot()`（当前 24–29 行）去掉对 `fixed_mutation` 的特例：

```typescript
function snapshot(t: ProtocolTimingConfig, o: RemoteOperationConfig): string {
  return JSON.stringify({ t, o })
}
```

- `saveAll()` 中删除当前 63–64 行：

```typescript
    await setFixedMutation({ ...ops.value.fixed_mutation })
    if (lastError.value) return
```

- 删 `startFixed` / `stopFixed`（当前 82–87 行）。
- 删模板 `#actions-fixed` 整块（当前 167–178 行 `<template #actions-fixed=...>…</template>`），使 `<RemoteParamsForm :timing="timing" :ops="ops" />` 自闭合。
- 删无用 CSS：`/* —— 固定变位启停 —— */` 起的 `.rp-fixed-actions`、`.rp-tag-btn`、`.rp-tag-start`、`.rp-tag-stop`、`.rp-pulse`、`.rp-fixed-state`、`.rp-state-dot`（当前约 383–427 行区间，逐一删除这些规则；保留其它无关样式）。

> 注:`.rp-tag-btn` 等仅服务固定变位启停。删除前用 `rg -n "rp-tag-btn|rp-fixed-state|rp-pulse|rp-state-dot|rp-fixed-actions" frontend/src/components/RemoteParamsDrawer.vue` 确认无模板再引用。

- [ ] **Step 6: RemoteParamsModal.vue — 删除固定变位接线与无用 CSS**

- 解构（当前第 25 行）去掉 `setFixedMutation`：

```typescript
const { timing, ops, loading, lastError, applyTiming, applyOps } =
  useRemoteParams(localServerId)
```

- `handleSave()` 删当前 39–40 行：

```typescript
    await setFixedMutation({ ...ops.value.fixed_mutation })
    if (lastError.value) return
```

- 删模板 `#actions-fixed` 整块（当前 84–94 行），使 `<RemoteParamsForm v-else :timing="timing" :ops="ops" />` 自闭合。
- 删无用 CSS：`.fixed-enable`、`.track`、`.thumb`、`.fixed-enable-label` 等仅服务该开关的规则（用 `rg -n "fixed-enable|\.track|\.thumb" frontend/src/components/RemoteParamsModal.vue` 定位后删除）。

- [ ] **Step 7: 前端构建确认通过**

Run: `npm --prefix frontend run build 2>&1 | tail -20`
Expected: 构建成功，无 `fixed_mutation` / `setFixedMutation` / `FixedMutationConfig` / `asduTypeOptions` 未定义或类型错误。

- [ ] **Step 8: Commit**

```bash
git add frontend/src/types.ts frontend/src/composables/useRemoteParams.ts \
  frontend/src/components/RemoteParamsForm.vue \
  frontend/src/components/RemoteParamsDrawer.vue \
  frontend/src/components/RemoteParamsModal.vue
git -c user.name="Karl-Dai Karl" -c user.email="kelsoprotein@gmail.com" \
  commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" \
  -m "refactor(slave-fe): 移除独立固定变位面板及其接线"
```

---

## Task 4: i18n 文案

**Files:**
- Modify: `frontend/src/i18n/locales/zh-CN.ts`
- Modify: `frontend/src/i18n/locales/en-US.ts`

- [ ] **Step 1: zh-CN.ts — 在 `table` 接口与值中加键**

接口块（当前第 141 行 `    deletePoint: string` 之后）加：

```typescript
    startMutation: string
    stopMutation: string
    mutationPeriod: string
```

值块（当前第 393 行 `    deletePoint: '删除数据点',` 之后）加：

```typescript
    startMutation: '启动周期变位',
    stopMutation: '停止周期变位',
    mutationPeriod: '周期',
```

- [ ] **Step 2: en-US.ts — 在 `table` 值中加键**

当前第 143 行 `    deletePoint: 'Delete Point',` 之后加：

```typescript
    startMutation: 'Start Mutation',
    stopMutation: 'Stop Mutation',
    mutationPeriod: 'Period',
```

> en-US 的 table 接口与 zh-CN 共用同一 interface（zh-CN.ts 中定义），Step 1 已补接口，无需在 en-US 重复声明类型。

- [ ] **Step 3: 构建确认 i18n 类型一致**

Run: `npm --prefix frontend run build 2>&1 | tail -20`
Expected: 构建成功（若 en-US 缺键会因接口要求而类型报错，本步应无报错）。

- [ ] **Step 4: Commit**

```bash
git add frontend/src/i18n/locales/zh-CN.ts frontend/src/i18n/locales/en-US.ts
git -c user.name="Karl-Dai Karl" -c user.email="kelsoprotein@gmail.com" \
  commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" \
  -m "i18n(slave-fe): 周期变位右键菜单文案"
```

---

## Task 5: 点表右键启停周期变位 + 脉冲指示

**Files:**
- Modify: `frontend/src/types.ts`
- Modify: `frontend/src/components/DataPointTable.vue`

- [ ] **Step 1: types.ts — 加 `PointMutationInfo` 类型**

在 `RemoteOperationConfig` 接口之后（当前约第 129 行后）加：

```typescript
/** list_point_mutations 返回项。asdu_type 为大写显示名（与 DataPointInfo.asdu_type 一致）。 */
export interface PointMutationInfo {
  ioa: number
  asdu_type: string
}
```

- [ ] **Step 2: DataPointTable.vue — script: 引入类型、活跃集状态与周期 ref**

import 行（当前第 6 行）补 `PointMutationInfo`：

```typescript
import type { DataPointInfo, IncrementalDataResponse, PointMutationInfo } from '../types'
```

在 `// === UI state ===` 区（当前约第 48 行 `changedKeys` 附近）加：

```typescript
// 当前 (server, CA) 下正在周期变位的点位 key 集合（`ioa:asdu_type`）。
const activeMutations = ref<Set<string>>(new Set())
// 右键菜单内的周期输入，组件内记住上次值。
const mutationPeriod = ref(1000)
```

- [ ] **Step 3: DataPointTable.vue — script: 活跃集刷新 + 启停函数**

在 `deleteSelectedPoints` 函数之后（当前约第 441 行后、`onPointAdded` 之前任意位置）加：

```typescript
// 拉取当前 (server, CA) 的活跃周期变位集合。
async function refreshActiveMutations() {
  const srvId = selectedServerId.value
  const ca = selectedCA.value
  if (!srvId || ca === null) { activeMutations.value = new Set(); return }
  try {
    const list = await invoke<PointMutationInfo[]>('list_point_mutations', {
      serverId: srvId,
      commonAddress: ca,
    })
    activeMutations.value = new Set(list.map(m => pointKey(m.ioa, m.asdu_type)))
  } catch (e) {
    console.error('Failed to load point mutations:', e)
  }
}

// 选中点位里是否有正在变位的（决定是否显示「停止」项）。
const anySelectedMutating = computed(() =>
  selectedRows.value.some(r => activeMutations.value.has(pointKey(r.ioa, r.asdu_type)))
)

async function startMutationForSelection() {
  contextMenu.value.show = false
  const srvId = selectedServerId.value
  if (!srvId || currentCA === null) return
  const period = Math.min(60000, Math.max(50, mutationPeriod.value || 1000))
  const targets = selectedRows.value.map(r => ({ ioa: r.ioa, asdu_type: r.asdu_type }))
  try {
    for (const tgt of targets) {
      await invoke('start_point_mutation', {
        serverId: srvId,
        commonAddress: currentCA,
        ioa: tgt.ioa,
        asduType: tgt.asdu_type,
        periodMs: period,
      })
    }
    await refreshActiveMutations()
  } catch (e) {
    await showAlert(String(e))
  }
}

async function stopMutationForSelection() {
  contextMenu.value.show = false
  const srvId = selectedServerId.value
  if (!srvId || currentCA === null) return
  const targets = selectedRows.value.map(r => ({ ioa: r.ioa, asdu_type: r.asdu_type }))
  try {
    for (const tgt of targets) {
      await invoke('stop_point_mutation', {
        serverId: srvId,
        commonAddress: currentCA,
        ioa: tgt.ioa,
        asduType: tgt.asdu_type,
      })
    }
    await refreshActiveMutations()
  } catch (e) {
    await showAlert(String(e))
  }
}
```

- [ ] **Step 4: DataPointTable.vue — script: 右键打开与轮询时刷新活跃集**

在 `showContextMenu`（当前约第 401 行）函数体最后一行 `contextMenu.value = { … }` 之后加：

```typescript
  refreshActiveMutations()
```

在 `startPolling` 的 interval 回调（当前约第 190–194 行）里，把 `loadDataPoints()` 一行改为：

```typescript
  pollTimer = setInterval(() => {
    if (currentServerId && currentCA !== null) {
      loadDataPoints()
      refreshActiveMutations()
    }
  }, 2000)
```

> 切站时 `loadDataPoints` 的 watcher 已重置数据；活跃集随首个 2s tick 与右键打开刷新即可，无需额外 watcher。

- [ ] **Step 5: DataPointTable.vue — 模板: 行的 mutating class 与 IOA 脉冲点**

行 `<tr>`（当前 522–531 行）的 `:class` 加 `mutating`：

```vue
            <tr
              v-for="point in visibleRows"
              :key="point.ioa + ':' + point.asdu_type"
              :class="{
                selected: isSelected(point),
                'value-changed': changedKeys.has(point.ioa + ':' + point.asdu_type),
                mutating: activeMutations.has(point.ioa + ':' + point.asdu_type)
              }"
              @click="selectRow($event, point)"
              @contextmenu.prevent="showContextMenu($event, point)"
            >
```

IOA 单元格（当前第 532 行）改为：

```vue
              <td class="col-ioa">
                <span v-if="activeMutations.has(point.ioa + ':' + point.asdu_type)" class="mut-dot" />{{ point.ioa }}
              </td>
```

- [ ] **Step 6: DataPointTable.vue — 模板: 右键菜单加入启停项与周期输入**

把右键菜单整块（当前 567–576 行）替换为：

```vue
    <div
      v-if="contextMenu.show"
      class="context-menu"
      :style="{ top: contextMenu.y + 'px', left: contextMenu.x + 'px' }"
      @click.stop
    >
      <label class="context-menu-period" @click.stop>
        <span>{{ t('table.mutationPeriod') }}</span>
        <input
          type="number"
          min="50"
          max="60000"
          v-model.number="mutationPeriod"
          @keydown.enter="startMutationForSelection"
          @click.stop
        />
        <span class="cm-unit">ms</span>
      </label>
      <div class="context-menu-item" @click="startMutationForSelection">
        {{ selectedCount > 1 ? `${t('table.startMutation')} (${selectedCount})` : t('table.startMutation') }}
      </div>
      <div v-if="anySelectedMutating" class="context-menu-item" @click="stopMutationForSelection">
        {{ t('table.stopMutation') }}
      </div>
      <div class="context-menu-sep" />
      <div class="context-menu-item danger" @click="deleteSelectedPoints">
        {{ selectedCount > 1 ? `${t('table.deletePoint')} (${selectedCount})` : t('table.deletePoint') }}
      </div>
    </div>
```

- [ ] **Step 7: DataPointTable.vue — CSS: 脉冲点、周期输入、分隔线**

在 `<style scoped>` 内 `.context-menu-item.danger` 相关规则（当前约 817–821 行）之后追加：

```css
.context-menu-period {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 12px;
  font-size: 12px;
  color: var(--c-subtext0);
}
.context-menu-period input {
  width: 64px;
  height: 22px;
  padding: 0 6px;
  background: var(--c-base);
  border: 1px solid var(--c-surface0);
  border-radius: 4px;
  color: var(--c-text);
  font: 500 12px/1 ui-monospace, "SF Mono", Menlo, monospace;
  text-align: right;
}
.context-menu-period input:focus {
  outline: none;
  border-color: var(--c-blue);
}
.cm-unit {
  color: var(--c-overlay0);
  font-size: 11px;
}
.context-menu-sep {
  height: 1px;
  margin: 4px 0;
  background: var(--c-surface0);
}
.mut-dot {
  display: inline-block;
  width: 6px;
  height: 6px;
  margin-right: 5px;
  border-radius: 50%;
  background: var(--c-green);
  vertical-align: middle;
  animation: mut-pulse 1s ease-in-out infinite;
}
@keyframes mut-pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.3; }
}
```

- [ ] **Step 8: 前端构建确认通过**

Run: `npm --prefix frontend run build 2>&1 | tail -20`
Expected: 构建成功，无类型/模板错误。

- [ ] **Step 9: Commit**

```bash
git add frontend/src/types.ts frontend/src/components/DataPointTable.vue
git -c user.name="Karl-Dai Karl" -c user.email="kelsoprotein@gmail.com" \
  commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" \
  -m "feat(slave-fe): 点表右键启停周期变位 + 脉冲指示"
```

---

## Task 6: 无头浏览器验证 + 全量回归

**Files:**
- 仅运行验证，无代码改动（除非发现缺陷）。

- [ ] **Step 1: 后端全量测试**

Run: `cargo test -p iec104sim-core 2>&1 | tail -15`
Expected: 全部 PASS。

- [ ] **Step 2: app crate 编译**

Run: `cargo check -p iec104sim-app 2>&1 | tail -10`
Expected: 通过。

- [ ] **Step 3: 前端构建**

Run: `npm --prefix frontend run build 2>&1 | tail -10`
Expected: 通过。

- [ ] **Step 4: Playwright 无头验证右键菜单与指示灯（遵循前端无头浏览器验证规则）**

用既有「注入 Tauri mock + vite 预览」套路（参考 `scripts/screenshots/capture.mjs`）写一次性脚本，mock `list_data_points_since`（返回含 IOA=1 / M_SP_NA_1 的点）、`list_point_mutations`（首次返回空，调用 `start_point_mutation` 后返回 `[{ioa:1, asdu_type:'M_SP_NA_1'}]`）、`start_point_mutation` / `stop_point_mutation`（记录调用并切换上面返回值）。无头浏览器中：

1. 选中 IOA=1 行 → 右键 → 断言菜单出现「启动周期变位」与「周期」输入。
2. 改周期为 500 → 点「启动周期变位」→ 断言 `start_point_mutation` 收到 `periodMs=500`、`asduType='M_SP_NA_1'`。
3. 断言该行出现 `.mut-dot`、行带 `mutating` class，且再次右键出现「停止周期变位」。
4. 点「停止周期变位」→ 断言 `stop_point_mutation` 被调用、`.mut-dot` 消失。

Run: `node <一次性脚本路径>`（无头、后台启动 vite preview，跑完即清理；不得弹出可见 GUI）。
Expected: 四项断言全过。

- [ ] **Step 5: 真机联调（可选但推荐）**

无头/后台启动子站 app 与一个主站连接，对真实点位右键启动 200ms 周期变位，确认主站侧收到周期 M_SP_NA_1 自发帧；停止后停。记录帧计数前后对比。

- [ ] **Step 6: 文档/Changelog（如项目惯例要求）**

若 `CHANGELOG.md`/`README_CN.md` 描述了原「固定变位」面板，更新为「点表右键周期变位」。否则跳过。

- [ ] **Step 7: 最终提交（如有验证脚本或文档改动）**

```bash
git add -A
git -c user.name="Karl-Dai Karl" -c user.email="kelsoprotein@gmail.com" \
  commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" \
  -m "test(slave): 周期变位无头验证脚本与文档更新"
```

---

## 注意事项

- **提交署名**:全部提交禁止 Claude 署名/Co-Authored-By，作者固定 `Karl-Dai Karl <kelsoprotein@gmail.com>`（见各 Commit 步骤的 `-c` / `--author`）。
- **iCloud 冲突**:`git add` 前若出现 `<name> 2.<ext>` 形式的 iCloud 冲突副本，先剔除。
- **不持久化**:周期变位为运行期仿真行为，`save_config`/`load_config` 不涉及，无需改动。
- **flip_value 语义不变**:测量值取负、开关量翻转、累计量 +1 均保持原样。
