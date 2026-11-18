# 104Master 广播公共地址(0xFFFF)总召唤设计

- 日期: 2026-05-28
- 范围: `crates/iec104master-app/`, `crates/iec104sim-core/`(master 接收路径与配置), `master-frontend/`
- 状态: 设计已澄清,等待用户审阅
- 相关材料: `/Users/daichangyu/Downloads/金风总召iectest.pcap`(CA=1 + QOI=20 的常规总召参考,**不是** 0xFFFF 广播实例)

## 1. 目标

为 104Master 增加 **广播公共地址(默认 0xFFFF,可配 0xFF00 等方言)** 的总召唤、对时、累计量召唤能力,同时:

- 广播召唤期间收到的未知 CA 自动补充到该连接的 `common_addresses`(去重 + 持久化)
- 现有"逐 CA fan-out"行为不动,周期 GI 也不动,广播作为**额外**手段
- 仅 master 侧;slave 当前不识别 0xFFFF/0xFF00,本设计**不动 slave**

## 2. 背景与现状

- `iec104sim-core::master` 的 `send_interrogation/clock_sync/counter_read` 都接受 `ca: u16`,**任意值都能出帧**——技术上传 `0xFFFF` 已经可行,只是没有人调。
- `master-frontend::Toolbar.sendGI()` 走 `fanOutCAs('send_interrogation')`,对配置里**每个 CA 串行发一帧 GI**,没有广播路径。
- 接收侧 `MasterReceivedData::insert(ca, point)` 用 `by_ca.entry(ca).or_default()`,**未知 CA 已经自动入桶**;无需重构存储层。
- `iec104sim-core::slave` 的 GI handler(`slave.rs` ~line 1080)只匹配 `stations_read.get(&ca)`,**收到 CA=0xFFFF 的 GI 会回 ActCon 但找不到 station 故不发数据**——这意味着本机 slave 自联自测验证不了广播,集成测试需要换路径。

## 3. 非目标

- 不为 slave 端加广播识别(列入 Future Work)
- 不重构 `send_interrogation` 函数签名(方案 B 已否)
- 不在前端做广播(方案 C 已否)
- 不聚合多从站 ActTerm 终止信号(每个从站各回各的 ActTerm,符合 IEC 60870-5-5 §6.10)

## 4. 架构

```
┌─ master-frontend ──────────────────────────────────────────────────┐
│ Toolbar.vue                                                         │
│   └─ "广播 ▾" 拆分按钮:                                              │
│        点击主体  → 广播总召                                          │
│        点击 ▾    → 弹出 3 选项 + tooltip 显示当前广播地址             │
│           ├─ 广播总召   → invoke('send_broadcast_gi')               │
│           ├─ 广播对时   → invoke('send_broadcast_clock_sync')        │
│           └─ 广播计量召 → invoke('send_broadcast_counter_read')      │
│ NewConnectionModal.vue                                              │
│   └─ 字段 "广播公共地址" (hex 文本框, 默认 FFFF, 4 位)               │
└────────────────────────────────────────────────────────────────────┘
                       ▼ Tauri IPC
┌─ iec104master-app/src/commands.rs ─────────────────────────────────┐
│ ConnectionRequest 加 broadcast_address: Option<u16>                 │
│ send_broadcast_gi(id)        → 读 conn.broadcast_address            │
│                              → conn.send_interrogation(addr)        │
│ send_broadcast_clock_sync(id)  同理                                 │
│ send_broadcast_counter_read(id) 同理                                │
└────────────────────────────────────────────────────────────────────┘
                       ▼
┌─ iec104sim-core/src/master.rs ─────────────────────────────────────┐
│ MasterConfig.broadcast_address: u16 (默认 0xFFFF, 任意 u16 透传)    │
│ 接收路径(~line 1761 解析出 ca 后):                                 │
│   if ca ∉ configured_cas && ca != broadcast_address:                │
│       new_ca_tx.send(ca)                                            │
│ CaDebouncer 后台 task:                                              │
│   3s 安静期触发一次 flush:                                          │
│     1) config.common_addresses 扩展去重                              │
│     2) emit Tauri 事件 "connection_updated"                         │
│   断连前强制 flush                                                  │
└────────────────────────────────────────────────────────────────────┘
```

## 5. 数据模型

### 5.1 后端

`MasterConfig`(`iec104sim-core/src/master.rs`)增加:
```rust
pub struct MasterConfig {
    // ...现有字段...
    /// 广播公共地址。用于广播 GI/对时/计量召唤。
    /// 默认 0xFFFF。常见替代值: 0xFF00。
    pub broadcast_address: u16,
}
```
- `Default::default()` 给 `0xFFFF`。
- 任意 u16 透传,不做白名单(应对厂商方言)。

`ConnectionRequest`(`iec104master-app/src/commands.rs`)增加:
```rust
pub broadcast_address: Option<u16>,
```
- `None` 时后端补默认 `0xFFFF`,兼容老配置文件。

### 5.2 前端

新建/编辑连接的请求/状态对象增加 `broadcast_address: number`(默认 65535)。
连接列表对象的 `common_addresses: number[]` 字段已有,debouncer flush 之后由后端发出新事件 `connection-cas-updated`(命名风格对齐既有 `connection-state` / `config-timing-corrected` 的 kebab-case)。前端 `App.vue` 加 `listen<{ id, common_addresses }>('connection-cas-updated', ...)`,处理:`refreshTree()` + 调一次 `save_connection`。

## 6. 未知 CA 自动补充(debouncer)

### 6.1 触发点

`master.rs` 接收路径解析出 `let ca = u16::from_le_bytes(...)` 之后(约 line 1761),在生成日志/插入数据之前:
```rust
if !self.config.common_addresses.contains(&ca) && ca != self.config.broadcast_address {
    let _ = self.new_ca_tx.send(ca);
}
```

### 6.2 Debouncer 状态机

后台 task(跟 `MasterClient` 同生命周期):
```
state: HashSet<u16>
quiet_deadline: Option<Instant>

收到 new_ca:
    state.insert(ca)
    quiet_deadline = Some(now + 3s)

每 100ms tick:
    if quiet_deadline.is_some_and(|d| now >= d) && !state.is_empty():
        flush(state.drain())
        quiet_deadline = None

flush(cas):
    1. config.common_addresses.extend(cas.iter().filter(去重))
    2. 通过 commands 层的 AppHandle 通道(回调或 mpsc) emit Tauri 事件
       "connection-cas-updated" { id, common_addresses }
       (kebab-case 对齐既有 "connection-state" / "config-timing-corrected")
```

注:`iec104sim-core::master` 不依赖 `tauri::AppHandle`(纯协议层)。debouncer 通过把 emit 闭包/`mpsc::Sender` 注入到 `MasterClient` 的方式回到 `iec104master-app`,实际 `app_handle.emit(...)` 调用发生在 commands 层,避免 core 层耦合 Tauri。

**3 秒安静期**:足够覆盖多从站对单帧广播 GI 的应答窗口;每次新 CA 出现重置计时。

**断连 hook**:断连流程在调用 `set_state(Disconnected)` 前,若 `state` 非空强制 flush 一次。

**协议错误兜底**:若从站把 `broadcast_address` 当自己 CA 回包,触发点过滤掉,不入 debouncer,日志记 `WARN`。

### 6.3 持久化

后端只更新内存中的 `MasterConfig.common_addresses` 并 emit 事件。**写盘留给前端**——前端 `connection_updated` 监听里 + 调一次 `save_connection`,保持"配置只在前端落盘"的既有边界(避免后端持久化耦合到 Tauri AppHandle)。

**iCloud 副本风险**(参考 memory `project_icloud_dup_files`):前端 `save_connection` 走的是项目既有路径,本设计不引入新的写盘代码,故无新增风险。

## 7. UI

### 7.1 工具栏

`Toolbar.vue` 新增 split button(可拆成 `BroadcastMenu.vue` 子组件):
```
[ 广播 ▾ ]
   ├─ 广播总召
   ├─ 广播对时
   └─ 广播计量召唤
```
- 主体点击 = 广播总召(最高频)
- ▾ 展开三选一菜单
- 按钮 tooltip:`当前广播地址: 0xFFFF`(或 0xFF00 时加角标提示方言)
- 断连或未选连接时整组禁用,与现有"总召"按钮一致

### 7.2 新建/编辑连接对话框

`NewConnectionModal.vue` 在通用段加一行:
```
广播公共地址: [FFFF]   hex
```
- input 类型:hex 文本框,4 位上限,大小写不敏感
- 空 → 默认 0xFFFF;非 hex / 超 4 位 → 红框 + 阻止保存
- 不强制白名单(允许任意 u16 包括 0、与已配 CA 重合等;后者 tooltip 加黄底警告)

### 7.3 i18n

`master-frontend/src/i18n/locales/{zh-CN,en-US}.ts` 增加:
- `toolbar.broadcast` / `toolbar.broadcastGi` / `toolbar.broadcastClockSync` / `toolbar.broadcastCounterRead`
- `connection.broadcastAddress` / `connection.broadcastAddressHint`

## 8. 错误与边界

| 场景 | 行为 |
|---|---|
| 未选连接就按广播按钮 | 按钮禁用 |
| 连接 Disconnected | 按钮禁用 |
| `broadcast_address = 0x0000` | 允许透传,不拒绝 |
| `broadcast_address` 与某个已配 CA 重合 | 允许,tooltip 黄底警告 |
| 广播期间断连 | 已采集到的新 CA 强制 flush;广播命令本身返回 send 错误,前端 toast |
| 从站用 `broadcast_address` 当自己 CA 回包 | 接收路径过滤,日志 WARN,不持久化 |
| 同一广播期间出现 50+ 新 CA | debouncer 单次 flush 全部,不分批 |
| 周期 GI 同时打开 | 不受影响,仍按 fan-out 走 |
| 配置文件无 `broadcast_address` 字段(老文件) | serde 默认 `None` → 后端补 `0xFFFF` |

## 9. 测试矩阵

### 9.1 单元测试(`crates/iec104sim-core/tests/`)

帧字节断言:
- `test_build_gi_command_broadcast_ffff` → CA 字节 `FF FF`
- `test_build_gi_command_broadcast_ff00` → CA 字节 `00 FF`(le)
- `test_build_clock_sync_broadcast_ffff`
- `test_build_counter_read_broadcast_ffff`

Debouncer:
- `test_debouncer_collects_and_flushes` —— 喂 3 CA,等 3.2 s,断言 flush 一次含 3 CA
- `test_debouncer_resets_on_new_ca` —— 第 1 CA 后 2 s 再来第 2 CA,从第 2 个起再算 3 s
- `test_debouncer_dedup` —— 同一 CA 喂 5 次,flush 只含一个
- `test_debouncer_skips_broadcast_addr` —— 喂 broadcast_address 本身,不入集合
- `test_debouncer_skips_configured` —— 喂已配置 CA,不入集合
- `test_debouncer_flush_on_shutdown` —— state 非空时强制 flush

### 9.2 集成测试(无头,遵循 `feedback_headless_testing`)

由于当前 slave 不识别 0xFFFF/0xFF00 广播(`stations_read.get(&ca)` 找不到 station),**集成测试不走"真 slave 自联"路径**。改为:

**A. 字节断言路径**(`crates/iec104sim-core/tests/`):
- 起 master 连本机 slave(slave CA=1)
- 在 master 端调 `send_interrogation(0xFFFF)`
- 用 `LogCollector` 抓 master 发出的 Tx 帧 raw_bytes,断言 CA 字节 = `FF FF`(或改 broadcast 后 `00 FF`)

**B. Mock fan-out 路径**(同一测试套件):
- 起两个 slave 站(CA=1, CA=2),分别在两个 IP/端口
- master 端"模拟" broadcast:对所有已知 IP 都发一次 `send_interrogation(1)` 和 `send_interrogation(2)`(临时绕过 slave 不识别 0xFFFF)
- 断言 master 端 `MasterReceivedData.cas()` = `[1, 2]`
- **注**:此项验证的是"接收侧聚合 + debouncer 持久化",不验证"slave 端真的认 0xFFFF";后者作为 Future Work。

**C. Debouncer 持久化路径**:
- 连接初始 `common_addresses=[1]`,手工往接收路径喂 ASDU 数据(`ca=99`)
- 等 ≥3.5 s,断言后端 `MasterConfig.common_addresses=[1, 99]` 且 `connection-cas-updated` 事件已触发

### 9.3 前端测试(Playwright,遵循 `feedback_frontend_headless_verify`)

- 启动 master GUI(headless)
- 新建连接 → 广播公共地址输入 `FF00` → 保存
- 模拟接入两个 slave 端口(CA=1, CA=2)
- 点工具栏"广播 ▾ → 广播总召"
- 等 3.5 s,截图断言连接树多出 CA=2 节点
- 修改广播地址回 `FFFF` → 再点广播 → 抓日志面板"原始字节"列断言起始包含 `FF FF`

## 10. 风险与未决

1. **slave 端不识别广播**——本次仅 master 侧;集成测试用字节断言 + mock fan-out 替代。slave 侧广播支持作为独立后续变更(Future Work)。
2. **持久化时机:debouncer 用 3 s 安静期**——若未来发现 3 s 过短(从站应答慢),把窗口做成 `MasterConfig.broadcast_settle_ms`(默认 3000)即可,本设计不预留该字段(YAGNI)。
3. **大量未知 CA 涌入**——50+ 一次性 flush 不分批;实测若日志面板卡顿,再加分批阈值(YAGNI)。
4. **iCloud 配置并发同步**——本设计不引入新的写盘代码,继承既有 `save_connection` 行为(参考 memory `project_icloud_git_dir_sync_hazard` / `project_icloud_reverts_edits`,无新增暴露面)。

## 11. 改动文件清单(预估)

| 文件 | 改动 |
|---|---|
| `crates/iec104sim-core/src/master.rs` | + `MasterConfig.broadcast_address`; + `new_ca_tx`/`CaDebouncer` 后台 task; ~ 接收路径加一行钩子; ~ 断连前强制 flush |
| `crates/iec104master-app/src/commands.rs` | + 3 个 Tauri 命令 `send_broadcast_*`; + `ConnectionRequest.broadcast_address` |
| `crates/iec104master-app/src/lib.rs` | + 注册 3 个 invoke handler |
| `master-frontend/src/components/Toolbar.vue` | + "广播 ▾" 按钮(可独立成 `BroadcastMenu.vue`) |
| `master-frontend/src/components/NewConnectionModal.vue` | + 广播地址 hex 字段 + 校验 |
| `master-frontend/src/i18n/locales/{zh-CN,en-US}.ts` | + 新文案 |
| `crates/iec104sim-core/tests/` | + 单测与集成测试 |
| `master-frontend/e2e/`(若存在,否则按现有 Playwright 路径) | + 广播按钮端到端用例 |

## 12. Future Work(明确不在本次变更内)

- slave 端识别 0xFFFF/0xFF00 广播,对其下挂所有 station 各回一组应答
- 主站 ActTerm 多从站聚合 UI(进度条/雷达图等)
- 自动学习从站方言广播地址(扫描 0xFF00 vs 0xFFFF 看哪个有响应)
