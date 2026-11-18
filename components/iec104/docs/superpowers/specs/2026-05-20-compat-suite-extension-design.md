# 互联回归套件扩展 — Design

- **状态**: Proposed
- **日期**: 2026-05-20
- **作者**: 自研栈 owner
- **关联仓库**:
  - 本仓库 `IEC60870-5-104-Simulator` (被测对象)
  - `../iec104-mzaniolo-ref` (mzaniolo/iec104 v0.4.0 参考实现, 本地 clone)
  - `../iec104-compat-suite` (本设计的产物, 本地 cargo workspace)

## 1. 背景

`iec104-compat-suite` 已经搭好双向 smoke (我们 master ↔ 第三方 RtuServer / 第三方 Client ↔ 我们 slave),证明 STARTDT + 总召 + 自发上送在两套独立 Rust 实现间互通。本 design 在此基础上将"对照机"价值最大化,扩展到 4 项工作:

1. **断言深度**: 把 smoke 的"数量断言"升级为"逐字段断言"(IOA 集合 + 值 + COT)。
2. **SBO 遥控**: 双向覆盖 Select→Execute→ActCon→ActTerm 时序。
3. **横向覆盖**: 多 CA / 设点 / TLS 各一组用例。
4. **CI 接入**: 本仓库 GitHub Actions workflow + checkout 三个 repo。

## 2. 目标 / 非目标

**目标**

- 用第三方 v0.4.0 作为对照机,持续验证 `iec104sim-core` 在主流协议路径上的规约一致性。
- 测试套件零侵入主仓库:不改 `iec104sim-core` 公开 API,不进 `Cargo.lock`。
- CI 在每日定时 + iec104sim-core 改动触发下自动跑,失败即可见。

**非目标**

- 不做"故意发非法报文"对抗用例 — 那是仿真器本身的卖点,跟"规约一致性背靠背"反向。
- 不做性能 / 长压 / fuzz — 噪声多,容易 flaky。
- 不验证 t1/t2/t3 超时 — 需要长 sleep,与单次跑 < 1 min 的 CI 目标冲突。
- 不替换本仓库自研栈;本设计纯属测试基础设施。
- 不写 IEC 101 / IEC 103 用例 — 第三方库范围仅 104。

## 3. 决策摘要

| 维度 | 决策 |
|------|------|
| spec 范围 | 单一 spec, 4 项一并 |
| 断言深度 | 中等: IOA 集合 + 值 + COT |
| TLS 证书 | 复用本仓库 `certs/` 已有 self-signed 套件 |
| CI 形式 | 本仓库 `.github/workflows/compat-suite.yml`, checkout 三个 repo |
| 第三方版本 | CI 钉 `ref: v0.4.0`, 升级显式 PR |
| 测试组织 | 按"协议特性"分文件 (smoke/sbo/setpoint/multi_ca/tls), helpers 抽公共启动+断言 |
| 平台 | ubuntu-latest 单平台 (matrix 留未来) |

## 4. 目录结构 (最终)

```
iec104-compat-suite/
├── Cargo.toml                    [workspace] + exclude 外部 path
├── README.md
├── .gitignore                    target/ Cargo.lock .DS_Store
└── crates/compat-tests/
    ├── Cargo.toml                dev-dep: iec104(path) + iec104sim-core(path)
    │                             + tokio + async-trait + anyhow
    ├── src/
    │   ├── lib.rs                pub mod helpers;
    │   └── helpers/
    │       ├── mod.rs            re-export
    │       ├── ports.rs          端口分配器
    │       ├── certs.rs          仓库 certs/ 解析 + native-tls 配置组装
    │       ├── asserts.rs        中等粒度断言 (IOA 集合 / 值 / COT)
    │       ├── thirdparty.rs     RtuServer / Client 启动工厂
    │       └── ours.rs           SlaveServer / MasterConnection 启动工厂
    └── tests/
        ├── smoke_bidirectional.rs    (升级版 smoke)
        ├── sbo_control.rs            SBO 双向
        ├── setpoint.rs               设点 NA/NB/NC 双向
        ├── multi_ca.rs               多 CA
        └── tls_handshake.rs          TLS 双向
```

本仓库内仅新增:

```
IEC60870-5-104-Simulator/
└── .github/workflows/compat-suite.yml
```

## 5. helpers 子模块设计

测试文件**只调 helpers**, 不直接碰 `iec104::*` / `iec104sim_core::*` 底层类型 — 上游 API 变化只在 helpers 一处维修。

### 5.1 `ports.rs`

```rust
const BASE: u16 = 22500;
static CURSOR: AtomicU16 = AtomicU16::new(0);

pub fn next_local_port() -> u16
```

`AtomicU16` 分配偏移 → probe-and-release 校验可用 → 占用则跳号。每个 test fn 通常拿 1 端口,多 CA 双实例用例拿 2 个。

### 5.2 `certs.rs`

```rust
pub fn repo_certs_dir() -> PathBuf       // ../../../IEC60870-5-104-Simulator/certs

pub struct TlsBundle {
    pub server_cert_pem: PathBuf,
    pub server_key_pem: PathBuf,
    pub client_cert_pem: PathBuf,
    pub client_key_pem: PathBuf,
    pub ca_pem: PathBuf,
}

pub fn load_bundle() -> TlsBundle
```

实现要点:
- `CARGO_MANIFEST_DIR` 起点 + `canonicalize()` 出错时给清晰路径报错。
- 启动时检查 5 个文件 `.exists()`, 缺失即 fail-fast。

### 5.3 `asserts.rs`

```rust
pub struct ExpectedPoint {
    pub ioa: u32,
    pub asdu_type: AsduTypeId,
    pub value: DataPointValue,
    pub allowed_cots: &'static [u8],   // 例如 [2, 20] 表示周期或总召响应
}

pub fn assert_points_match(
    received: &MasterReceivedData,
    ca: u16,
    expected: &[ExpectedPoint],
);
```

失败 diff 格式:
```
IOA 集合不匹配:
  缺失: [40, 41]
  多余: []
  值不同:
    IOA=11 type=M_ME_NC_1: 期望 42.0, 实际 41.999996 (epsilon 超出 1e-6)
  COT 不同:
    IOA=20: 期望 [2, 20], 实际 [3]
```

浮点比较用 `(a - b).abs() <= 1e-6 * a.abs().max(1.0)`。COT 允许集合方式判定,因为同一点可能在不同时机被多个 COT 触发。

反向(第三方 Client 收到我们 slave 的 ASDU)的断言走 `CountingCallback` 把 `Asdu` clone 进 Vec, 然后用 `assert_third_party_asdus(received: &[Asdu], expected: &[...])` 做对称比较。

### 5.4 `thirdparty.rs`

```rust
pub async fn spawn_rtu_server(
    port: u16,
    initial_points: Vec<RtuInitialPoint>,
    tls: Option<TlsServerConfig>,
) -> RtuServerHandle;

pub struct CapturingCallback { pub asdus: Arc<Mutex<Vec<Asdu>>>, pub started: AtomicUsize, pub errors: AtomicUsize }

pub async fn spawn_client(
    port: u16,
    tls: Option<TlsClientConfig>,
    callback: CapturingCallback,
) -> Client<...>
```

`RtuServerHandle` 实现 `Drop` 关闭服务;无显式 stop 需求。

### 5.5 `ours.rs`

```rust
pub async fn spawn_slave(
    port: u16,
    stations: Vec<Station>,
    tls: Option<SlaveTlsConfig>,
) -> SlaveServer;

pub async fn spawn_master(
    port: u16,
    ca: u16,
    tls: Option<TlsConfig>,
) -> MasterConnection;

pub fn tls_config_for_master(bundle: &TlsBundle) -> TlsConfig;
pub fn tls_config_for_slave(bundle: &TlsBundle) -> SlaveTlsConfig;
```

`spawn_*` 完成"new + start/connect + 等待状态稳定"一条龙,测试代码再不写 sleep 兜底。

## 6. 测试用例矩阵

11 个 `#[tokio::test]`, 全部用 `flavor = "multi_thread", worker_threads = 4`。

| 文件 | test fn | 方向 | 验证点 |
|------|---------|------|--------|
| smoke_bidirectional.rs | `our_master_total_interrogates_thirdparty` | 我们 M → 第三方 S | IOA 集合 = {1,11,20,21,30,40};IOA=11 ≈ 42.0;COT∈{2,20} |
| smoke_bidirectional.rs | `thirdparty_master_receives_spontaneous` | 第三方 M → 我们 S | callback 收到 ≥1 ASDU,TypeID=M_SP_NA_1,IOA=1,COT=3 |
| sbo_control.rs | `our_master_sbo_to_thirdparty_slave` | 我们 M → 第三方 S | ActCon S/E=1, ActCon S/E=0, ActTerm 可选;P/N=0 |
| sbo_control.rs | `thirdparty_master_sbo_to_our_slave` | 第三方 M → 我们 S | ActCon×2 + ActTerm;COT 序列 7/7/10;P/N=0 |
| setpoint.rs | `setpoint_normalized_round_trip` | 我们 M → 第三方 S | ActCon + IOA=20 NVA 与发送一致 |
| setpoint.rs | `setpoint_scaled_round_trip` | 我们 M → 第三方 S | IOA=21 SVA 一致 |
| setpoint.rs | `setpoint_float_round_trip` | 我们 M → 第三方 S | IOA=11 浮点 epsilon ≤ 1e-6 |
| multi_ca.rs | `our_slave_serves_two_cas` | 第三方 M → 我们 S | 我们 slave 注入 CA=10/20 各 2 点;两次总召分别命中各自 IOA |
| multi_ca.rs | `our_master_polls_two_cas` | 我们 M → 第三方 S × 2 | 双 RtuServer 实例 (端口 A/B, CA 47/48);`received_data.by_ca` 两 CA 各自完整 |
| tls_handshake.rs | `our_master_tls_to_thirdparty_slave` | 我们 M → 第三方 S (TLS) | TLS 握手 + 总召 ≥ 2 点 |
| tls_handshake.rs | `thirdparty_master_tls_to_our_slave` | 第三方 M → 我们 S (TLS) | TLS 握手 + STARTDT_CON + 自发上送 ≥ 1 |

每个 test fn 调一次 `ports::next_local_port()` 拿独立端口,绝不并发碰撞。

## 7. 端口分配 & 并发隔离

见 §5.1。补充约束:

- BASE=22500 起步, 22404/22405 留给现有 smoke (升级后会改用 next_local_port,但 BASE 仍避开)。
- 每个测试独立 tokio runtime (`#[tokio::test(flavor="multi_thread")]`)。
- helpers 默认不装 `tracing_subscriber`,避免并发日志互相覆盖;`RUST_LOG` 显式时才装,且用 `Once` 防重复。
- `cargo test --test-threads 2` 在 CI 强制 2 路并发,本地默认按 CPU 核数。

## 8. TLS 用例细节

### 8.1 证书复用

复用 `IEC60870-5-104-Simulator/certs/` 的 8 个文件 (ca + server + client, PEM + p12 + key)。

- 第三方 server 用 `TlsServerConfig { server_certificate: server.pem, server_key: server-key.pem }`.
- 第三方 client 用 `TlsClientConfig { server_certificate: Some(ca.pem), .. }`.
- 我们 master/slave 走现有 `TlsConfig` / `SlaveTlsConfig` (字段在 implementation 期间核对)。

### 8.2 SAN 兜底

如果 `server.pem` 的 SAN 不包含 `127.0.0.1`, 第三方 client 默认 verify 会失败。helpers 决策:

1. 优先尝试连 `localhost`(loopback DNS 通常成立 + 证书 CN 多为 localhost)。
2. 若仍失败, 测试在 `TlsClientConfig.danger_disable_tls_verify = true` 显式跳过校验, 测试名加 `_insecure` 后缀标注。
3. spec 落地时由 implementation 第一步实测 1)/2) 哪个成立,决定最终走哪条。**不预设**。

### 8.3 证书过期检查

implementation 第一个 task 跑一次:

```bash
openssl x509 -in certs/server.pem -noout -enddate
openssl x509 -in certs/ca.pem -noout -enddate
```

如 12 个月内过期, plan 里加一个"用 rcgen / openssl 重签"task; 否则跳过。

## 9. CI workflow

文件: `IEC60870-5-104-Simulator/.github/workflows/compat-suite.yml`.

```yaml
name: compat-suite
on:
  push:
    branches: [main]
    paths:
      - 'crates/iec104sim-core/**'
      - '.github/workflows/compat-suite.yml'
  pull_request:
    paths:
      - 'crates/iec104sim-core/**'
  schedule:
    - cron: '0 18 * * *'   # 北京 02:00
  workflow_dispatch: {}

jobs:
  compat-smoke:
    runs-on: ubuntu-latest
    timeout-minutes: 15
    steps:
      - uses: actions/checkout@v4
        with: { path: IEC60870-5-104-Simulator }
      - uses: actions/checkout@v4
        with:
          repository: mzaniolo/iec104
          ref: v0.4.0
          path: iec104-mzaniolo-ref
      - uses: actions/checkout@v4
        with:
          repository: <your-org>/iec104-compat-suite
          ref: main
          path: iec104-compat-suite
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
        with: { workspaces: iec104-compat-suite }
      - working-directory: iec104-compat-suite
        env: { RUST_BACKTRACE: 1 }
        run: cargo test --all -- --test-threads 2
```

`<your-org>` 在 push 前替换。implementation 最后一步会建 repo 并替换。

## 10. 错误处理 & 失败诊断

- `wait_until` / `wait_for_value` 超时返回 `Err(())`, `.expect("场景描述")` 给 panic 可读信息。
- `assert_points_match` 失败 message 包含 missing/extra/diff 三类一次性 dump。
- `CapturingCallback` 暴露 `errors` 计数, 测试结束断言 `== 0`, 否则带 error 列表 fail。
- 默认无 `tracing` 输出, 失败时用 `RUST_LOG=debug cargo test ... -- --nocapture` 重跑。
- 测试名前缀方向(`our_master_*` / `thirdparty_master_*`), GitHub Actions UI 一眼定位故障侧。

## 11. 风险登记

| 风险 | 等级 | 缓解 |
|------|------|------|
| 第三方库 0.5.0+ API 破坏 | 中 | CI `ref: v0.4.0` 钉死;升级走显式 PR |
| 证书 SAN 不含 127.0.0.1 致 TLS 握手失败 | 中 | helpers SAN 兜底 (localhost / insecure 二选一);impl 第一步实测 |
| iCloud 路径 `canonicalize` 怪行为 | 低 | fail-fast 报清晰路径 |
| 我们 slave 多 CA 处理差异 | 中 | 测试本身验证;差异即 bug |
| 端口冲突 | 低 | probe-and-release + 22500 起步 |
| GitHub Actions flaky | 低 | `timeout-minutes: 15` + 失败 re-run |
| **第三方 SBO 实现细节差异** | **高** | 最可能首次失败的项;失败首选定位 + 写 issue, **不立刻改协议层** |
| compat-suite repo 尚未建 | 低 | impl 最后一步建, 空仓库 push |
| 证书 12 个月内过期 | 低 | impl 第一步 openssl 检查 |

## 12. 完成定义 (DoD)

1. 11 个测试函数全部存在且实现完成。
2. `cd iec104-compat-suite && cargo test --all` 在本地 macOS 上**全绿**。
3. 预测会失败的用例 (SBO / 多 CA / TLS) 若实际失败:
   - 在 `iec104-compat-suite/FAILURES.md` 登记差异(IOA / COT / 字节级根因);
   - 在本仓库 issue tracker 起 follow-up 单(若是我们协议层 bug);
   - **不在本轮 spec 范围内修复协议层** — 测试套件的产出是发现差异本身,而不是消除差异。
4. `.github/workflows/compat-suite.yml` 写入本仓库, `actionlint` 通过或一次 CI 实跑无语法错。
5. `iec104-compat-suite` 已建 GitHub repo + 首次 push 完成。
6. workflow 中 `<your-org>` 占位符已替换为实际值。

## 13. 后续 (out-of-scope, 仅登记)

- 加 windows-latest / macos-latest matrix。
- 加 `iec104-mzaniolo-ref` 的 `ref: main` 双轨 job(钉版本 + 跟踪 main), 后者 fail 不阻塞。
- 加 t1/t2/t3 超时用例 (需要长 sleep, 走单独 nightly job)。
- 引入 `cargo-nextest` 加速。
- 加"故意发非法报文"对抗用例 — 验证我们 slave 在第三方 client 视角下是否被识别为"非合规但可恢复"。
- IEC 101 串口对比 (需要其它纯 Rust 库, 目前生态不足)。

## 14. 实施 (留给 writing-plans)

本 spec 由 brainstorming 输出, 后续走 `writing-plans` skill 生成分 task 的实施计划。Plan 中的 task 顺序建议:

1. 证书过期检查 + SAN 兜底实测
2. helpers/ports + helpers/asserts + helpers/certs
3. helpers/thirdparty + helpers/ours
4. 升级 smoke_bidirectional.rs (现有 2 个用例 → 中等粒度断言)
5. 新增 sbo_control.rs (2 用例)
6. 新增 setpoint.rs (3 用例)
7. 新增 multi_ca.rs (2 用例)
8. 新增 tls_handshake.rs (2 用例)
9. 本地 `cargo test --all` 全绿验证
10. 写 .github/workflows/compat-suite.yml
11. GitHub 建 iec104-compat-suite repo + 首次 push
12. 替换 workflow 中 `<your-org>` + 触发首次 CI 跑
13. 登记 follow-up (如有失败用例)
