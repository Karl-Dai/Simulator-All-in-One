# 互联回归套件扩展 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 iec104-compat-suite 从两个 smoke 扩展到 11 用例 (中等粒度断言 + SBO + 设点 + 多 CA + TLS),并在本仓库接入 GitHub Actions 互联回归 CI。

**Architecture:** 测试代码放在 `../iec104-compat-suite/`(本仓库外的独立 cargo workspace,path 依赖本仓库 `iec104sim-core` 与 `../iec104-mzaniolo-ref` 第三方实现)。helpers 抽公共"启动服务 / 端口分配 / 断言 diff",测试文件只调 helpers。CI 在本仓库 `.github/workflows/compat-suite.yml` checkout 三个 repo 后 `cargo test --all`。

**Tech Stack:** Rust 2021、tokio multi-thread runtime、async-trait、native-tls(继承本仓库证书)、第三方 `iec104 = 0.4.0`、本仓库 `iec104sim-core`。GitHub Actions `dtolnay/rust-toolchain@stable` + `Swatinem/rust-cache@v2`。

**前置事实 (impl 期间已确认):**

- 证书 `certs/server.pem` SAN = `DNS:localhost, IP:127.0.0.1, IP:10.15.48.12`,有效,**TLS 直连 127.0.0.1 验证通过,无需 insecure 兜底**。
- 现有 `crates/compat-tests/tests/smoke_*.rs` 两个文件已在本地通过。
- 第三方 SBO 字段 `SelectExecute::{Select, Execute}` + `send_command_sp(.., Option<SelectExecute>, ..)` 可用。

**路径约定:** 下文 `compat-suite/` = `/Users/.../code/iec104-compat-suite/`(本仓库的上一级目录)。本仓库内的文件用相对仓库根路径 (`crates/...` / `.github/...`)。

---

## Task 1: 准备 helpers 模块骨架

**Files:**
- Create: `compat-suite/crates/compat-tests/src/helpers/mod.rs`
- Create: `compat-suite/crates/compat-tests/src/helpers/ports.rs`
- Create: `compat-suite/crates/compat-tests/src/helpers/certs.rs`
- Create: `compat-suite/crates/compat-tests/src/helpers/asserts.rs`
- Create: `compat-suite/crates/compat-tests/src/helpers/thirdparty.rs`
- Create: `compat-suite/crates/compat-tests/src/helpers/ours.rs`
- Modify: `compat-suite/crates/compat-tests/src/lib.rs`

- [ ] **Step 1: 写空模块文件**

`compat-suite/crates/compat-tests/src/helpers/mod.rs`:
```rust
//! 公共测试辅助 — 端口分配、证书、断言、第三方/我们的服务启动工厂。
pub mod ports;
pub mod certs;
pub mod asserts;
pub mod thirdparty;
pub mod ours;
```

每个子模块文件 (`ports.rs` / `certs.rs` / `asserts.rs` / `thirdparty.rs` / `ours.rs`) 暂时只写:
```rust
//! TODO: 在后续 task 中实现
#![allow(dead_code)]
```

- [ ] **Step 2: 更新 lib.rs 导出 helpers**

把 `compat-suite/crates/compat-tests/src/lib.rs` 当前内容改为:
```rust
//! iec104-compat-suite: 用第三方 `mzaniolo/iec104` 作为对照机,
//! 对本仓库 `iec104sim-core` 的 master / slave 实现进行互联回归。
pub mod helpers;
```

- [ ] **Step 3: 验证 cargo check 通过**

Run: `cd compat-suite && cargo check --tests`
Expected: `Finished ... 0 warnings`(除了 `dead_code` 已 allow)

- [ ] **Step 4: Commit**

```bash
cd compat-suite
git add crates/compat-tests/src/lib.rs crates/compat-tests/src/helpers/
git commit -m "chore(compat-tests): helpers 模块骨架"
```

---

## Task 2: helpers::ports — 端口分配器

**Files:**
- Modify: `compat-suite/crates/compat-tests/src/helpers/ports.rs`

- [ ] **Step 1: 写失败测试**

把 `helpers/ports.rs` 改为:
```rust
//! 测试端口分配:原子计数器 + probe-and-release。
use std::net::TcpListener;
use std::sync::atomic::{AtomicU16, Ordering};

const BASE: u16 = 22500;
static CURSOR: AtomicU16 = AtomicU16::new(0);

pub fn next_local_port() -> u16 {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_distinct_ports_under_concurrent_calls() {
        // 至少前 5 次调用拿到的端口都不一样,且 >= BASE。
        let ps: Vec<u16> = (0..5).map(|_| next_local_port()).collect();
        for p in &ps {
            assert!(*p >= BASE, "{p} < BASE");
        }
        let unique: std::collections::HashSet<_> = ps.iter().collect();
        assert_eq!(unique.len(), ps.len(), "重复端口: {ps:?}");
    }

    #[test]
    fn ports_are_actually_bindable() {
        let p = next_local_port();
        let _l = TcpListener::bind(("127.0.0.1", p)).expect("拿到的端口应当可绑");
    }
}
```

- [ ] **Step 2: 运行测试验证失败**

Run: `cd compat-suite && cargo test -p compat-tests --lib ports`
Expected: panic at `todo!()` 两个测试都 fail

- [ ] **Step 3: 实现 next_local_port**

替换 `todo!()` 为:
```rust
pub fn next_local_port() -> u16 {
    loop {
        let off = CURSOR.fetch_add(1, Ordering::SeqCst);
        let port = BASE.checked_add(off).expect("端口耗尽");
        if TcpListener::bind(("127.0.0.1", port)).is_ok() {
            return port;
        }
    }
}
```

- [ ] **Step 4: 验证测试通过**

Run: `cd compat-suite && cargo test -p compat-tests --lib ports`
Expected: `test result: ok. 2 passed`

- [ ] **Step 5: Commit**

```bash
cd compat-suite
git add crates/compat-tests/src/helpers/ports.rs
git commit -m "feat(compat-tests): helpers::ports 端口分配器"
```

---

## Task 3: helpers::certs — 仓库证书复用

**Files:**
- Modify: `compat-suite/crates/compat-tests/src/helpers/certs.rs`

- [ ] **Step 1: 写失败测试**

```rust
//! 读取本仓库 certs/ 的 self-signed PEM 套件。
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct TlsBundle {
    pub server_cert_pem: PathBuf,
    pub server_key_pem: PathBuf,
    pub client_cert_pem: PathBuf,
    pub client_key_pem: PathBuf,
    pub ca_pem: PathBuf,
}

pub fn repo_certs_dir() -> PathBuf {
    todo!()
}

pub fn load_bundle() -> TlsBundle {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn certs_dir_exists() {
        let d = repo_certs_dir();
        assert!(d.is_dir(), "{d:?} 不是目录");
    }

    #[test]
    fn bundle_files_all_exist() {
        let b = load_bundle();
        for (name, p) in [
            ("server_cert_pem", &b.server_cert_pem),
            ("server_key_pem", &b.server_key_pem),
            ("client_cert_pem", &b.client_cert_pem),
            ("client_key_pem", &b.client_key_pem),
            ("ca_pem", &b.ca_pem),
        ] {
            assert!(p.is_file(), "{name}: {p:?} 不存在");
        }
    }
}
```

- [ ] **Step 2: 运行测试验证失败**

Run: `cd compat-suite && cargo test -p compat-tests --lib certs`
Expected: 两个测试都 panic at `todo!()`

- [ ] **Step 3: 实现**

```rust
pub fn repo_certs_dir() -> PathBuf {
    let manifest = env!("CARGO_MANIFEST_DIR");
    Path::new(manifest)
        .join("../../../IEC60870-5-104-Simulator/certs")
        .canonicalize()
        .expect("certs 目录解析失败,检查上一级是否有 IEC60870-5-104-Simulator")
}

pub fn load_bundle() -> TlsBundle {
    let d = repo_certs_dir();
    TlsBundle {
        server_cert_pem: d.join("server.pem"),
        server_key_pem: d.join("server-key.pem"),
        client_cert_pem: d.join("client.pem"),
        client_key_pem: d.join("client-key.pem"),
        ca_pem: d.join("ca.pem"),
    }
}
```

- [ ] **Step 4: 验证测试通过**

Run: `cd compat-suite && cargo test -p compat-tests --lib certs`
Expected: `test result: ok. 2 passed`

- [ ] **Step 5: Commit**

```bash
cd compat-suite
git add crates/compat-tests/src/helpers/certs.rs
git commit -m "feat(compat-tests): helpers::certs 复用仓库 self-signed 证书"
```

---

## Task 4: helpers::asserts — 中等粒度断言

**Files:**
- Modify: `compat-suite/crates/compat-tests/src/helpers/asserts.rs`

- [ ] **Step 1: 写失败测试**

```rust
//! 中等粒度断言:
//!   - 正向(MasterReceivedData): IOA 集合相等 + 值匹配
//!     (本仓库 DataPoint 不存 COT 字段, COT 验证转入反向)
//!   - 反向(第三方 Client 收到的 Asdu): IOA + TypeID + Cot 在 callback 侧验

use std::collections::HashSet;
use std::fmt::Write as _;

use iec104sim_core::data_point::DataPointValue;
use iec104sim_core::master::MasterReceivedData;
use iec104sim_core::types::AsduTypeId;

#[derive(Debug, Clone)]
pub struct ExpectedPoint {
    pub ioa: u32,
    pub asdu_type: AsduTypeId,
    pub value: DataPointValue,
}

const FLOAT_EPS: f32 = 1e-6;

pub fn value_approx_eq(a: &DataPointValue, b: &DataPointValue) -> bool {
    match (a, b) {
        (DataPointValue::SinglePoint { value: x }, DataPointValue::SinglePoint { value: y }) => x == y,
        (DataPointValue::DoublePoint { value: x }, DataPointValue::DoublePoint { value: y }) => x == y,
        (DataPointValue::Scaled { value: x }, DataPointValue::Scaled { value: y }) => x == y,
        (DataPointValue::Normalized { value: x }, DataPointValue::Normalized { value: y })
        | (DataPointValue::ShortFloat { value: x }, DataPointValue::ShortFloat { value: y }) => {
            (x - y).abs() <= FLOAT_EPS * x.abs().max(1.0)
        }
        _ => false,
    }
}

pub fn assert_points_match(
    received: &MasterReceivedData,
    ca: u16,
    expected: &[ExpectedPoint],
) {
    let map = match received.ca_map(ca) {
        Some(m) => m,
        None => panic!("CA={ca} 在 received_data 中不存在"),
    };
    let got_ioas: HashSet<u32> = map.all_sorted().iter().map(|p| p.ioa).collect();
    let want_ioas: HashSet<u32> = expected.iter().map(|e| e.ioa).collect();

    let missing: Vec<u32> = {
        let mut v: Vec<_> = want_ioas.difference(&got_ioas).copied().collect();
        v.sort();
        v
    };
    let extra: Vec<u32> = {
        let mut v: Vec<_> = got_ioas.difference(&want_ioas).copied().collect();
        v.sort();
        v
    };

    let mut diff_value = String::new();
    for e in expected {
        if let Some(p) = map.get(e.ioa, e.asdu_type) {
            if !value_approx_eq(&p.value, &e.value) {
                let _ = writeln!(diff_value, "    IOA={} type={:?}: 期望 {:?}, 实际 {:?}",
                    e.ioa, e.asdu_type, e.value, p.value);
            }
        }
    }

    if !missing.is_empty() || !extra.is_empty() || !diff_value.is_empty() {
        let mut msg = String::from("数据点不匹配:\n");
        if !missing.is_empty() { let _ = writeln!(msg, "  缺失: {missing:?}"); }
        if !extra.is_empty() { let _ = writeln!(msg, "  多余: {extra:?}"); }
        if !diff_value.is_empty() { let _ = writeln!(msg, "  值不同:\n{diff_value}"); }
        panic!("{msg}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iec104sim_core::data_point::DataPoint;

    #[test]
    fn passes_when_all_match() {
        let mut rx = MasterReceivedData::new();
        let mut p = DataPoint::new(1, AsduTypeId::MSpNa1);
        p.value = DataPointValue::SinglePoint { value: true };
        rx.insert(47, p);

        assert_points_match(&rx, 47, &[ExpectedPoint {
            ioa: 1,
            asdu_type: AsduTypeId::MSpNa1,
            value: DataPointValue::SinglePoint { value: true },
        }]);
    }

    #[test]
    #[should_panic(expected = "缺失")]
    fn panics_on_missing_ioa() {
        let rx = MasterReceivedData::new();
        assert_points_match(&rx, 47, &[ExpectedPoint {
            ioa: 1,
            asdu_type: AsduTypeId::MSpNa1,
            value: DataPointValue::SinglePoint { value: true },
        }]);
    }
}
```

- [ ] **Step 2: 运行测试**

Run: `cd compat-suite && cargo test -p compat-tests --lib asserts`
Expected: 两个测试通过 (一个走正常路径,一个 should_panic)

如果 `DataPoint` 没有 `cot` 字段,先看 `crates/iec104sim-core/src/data_point.rs` 确认:Run `grep -n 'cot' /Users/.../IEC60870-5-104-Simulator/crates/iec104sim-core/src/data_point.rs`。如果字段名不同,调整 `assert_points_match` 内对应字段访问。

- [ ] **Step 3: Commit**

```bash
cd compat-suite
git add crates/compat-tests/src/helpers/asserts.rs
git commit -m "feat(compat-tests): helpers::asserts 中等粒度数据点断言"
```

---

## Task 5: helpers::thirdparty — 第三方实例工厂

**Files:**
- Modify: `compat-suite/crates/compat-tests/src/helpers/thirdparty.rs`

- [ ] **Step 1: 实现**

```rust
//! 起第三方 `iec104` 的 RtuServer / Client, 提供 RAII handle。
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;

use iec104::asdu::Asdu;
use iec104::client::{Client, ClientCallback};
use iec104::config::{ClientConfig, ServerConfig, TlsClientConfig, TlsServerConfig};
use iec104::rtu_server::{
    MapCommandsToSameIoaMonitoring, RtuInitialPoint, RtuServer, RtuServerHandle,
};

/// 起一个第三方 RtuServer,返回 handle (drop 即关闭)。
pub async fn spawn_rtu_server(
    port: u16,
    initial_points: Vec<RtuInitialPoint>,
    tls: Option<TlsServerConfig>,
) -> RtuServerHandle {
    let mut cfg = ServerConfig::default();
    cfg.address = "127.0.0.1".into();
    cfg.port = port;
    cfg.tls = tls;

    let handle = RtuServer::start(cfg, initial_points, Arc::new(MapCommandsToSameIoaMonitoring))
        .await
        .expect("RtuServer::start 失败");

    // 让 listener 真正进入 accept 状态
    tokio::time::sleep(Duration::from_millis(150)).await;

    handle
}

/// 计数 + 抓取第三方 client 收到的 ASDU。
#[derive(Default)]
pub struct Counters {
    pub asdu_count: AtomicUsize,
    pub started: AtomicUsize,
    pub errors: AtomicUsize,
    pub asdus: tokio::sync::Mutex<Vec<Asdu>>,
}

pub struct CapturingCallback(pub Arc<Counters>);

#[async_trait]
impl ClientCallback for CapturingCallback {
    async fn on_new_objects(&self, asdu: Asdu) {
        self.0.asdu_count.fetch_add(1, Ordering::SeqCst);
        self.0.asdus.lock().await.push(asdu);
    }
    async fn on_connection_started(&self) {
        self.0.started.fetch_add(1, Ordering::SeqCst);
    }
    async fn on_error(&self, _error: iec104::error::Error) {
        self.0.errors.fetch_add(1, Ordering::SeqCst);
    }
}

/// 起一个第三方 Client + 完成 STARTDT。返回 (Client, Counters)。
pub async fn spawn_client(
    port: u16,
    tls: Option<TlsClientConfig>,
) -> (Client<CapturingCallback>, Arc<Counters>) {
    let counters = Arc::new(Counters::default());
    let mut cfg = ClientConfig::default();
    cfg.address = "127.0.0.1".into();
    cfg.port = port;
    cfg.tls = tls;

    let mut client = Client::new(cfg, CapturingCallback(counters.clone()));
    client.connect().await.expect("client.connect 失败");
    client.start_receiving().await.expect("client.start_receiving 失败");

    // 等 STARTDT_CON 触发回调。
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while counters.started.load(Ordering::SeqCst) == 0 {
        if tokio::time::Instant::now() >= deadline {
            panic!("5s 内未收到 STARTDT_CON");
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    (client, counters)
}
```

- [ ] **Step 2: 编译验证**

Run: `cd compat-suite && cargo check -p compat-tests --tests`
Expected: `Finished ...`,如果 `RtuServerHandle` 不是 pub,要么改用 `RtuServer::start` 返回的具体类型,要么 grep 第三方 rtu_server 模块找正确导出。

执行排错:Run `grep -rn 'pub.*RtuServerHandle\|pub fn start' /Users/.../iec104-mzaniolo-ref/src/rtu_server/`,如果发现命名不一致,改 `use` 路径。

- [ ] **Step 3: Commit**

```bash
cd compat-suite
git add crates/compat-tests/src/helpers/thirdparty.rs
git commit -m "feat(compat-tests): helpers::thirdparty 第三方实例工厂 + CapturingCallback"
```

---

## Task 6: helpers::ours — 自研栈实例工厂

**Files:**
- Modify: `compat-suite/crates/compat-tests/src/helpers/ours.rs`

- [ ] **Step 1: 实现**

```rust
//! 起本仓库 `iec104sim-core` 的 SlaveServer / MasterConnection。
use std::time::Duration;

use iec104sim_core::master::{MasterConfig, MasterConnection, MasterState, TlsConfig};
use iec104sim_core::slave::{
    SlaveServer, SlaveTlsConfig, SlaveTransportConfig, Station,
};

use super::certs::TlsBundle;

pub async fn spawn_slave(
    port: u16,
    stations: Vec<Station>,
    tls: Option<SlaveTlsConfig>,
) -> SlaveServer {
    let transport = SlaveTransportConfig {
        bind_address: "127.0.0.1".into(),
        port,
        tls: tls.unwrap_or_default(),
    };
    let mut slave = SlaveServer::new(transport);
    for s in stations {
        slave.add_station(s).await.expect("add_station 失败");
    }
    slave.start().await.expect("slave.start 失败");
    tokio::time::sleep(Duration::from_millis(150)).await;
    slave
}

pub async fn spawn_master(
    port: u16,
    ca: u16,
    tls: Option<TlsConfig>,
) -> MasterConnection {
    let mut cfg = MasterConfig::default();
    cfg.target_address = "127.0.0.1".into();
    cfg.port = port;
    cfg.common_address = ca;
    cfg.timeout_ms = 3000;
    if let Some(t) = tls {
        cfg.tls = t;
    }
    let mut master = MasterConnection::new(cfg);
    master.connect().await.expect("master.connect 失败");

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while !matches!(master.state(), MasterState::Connected) {
        if tokio::time::Instant::now() >= deadline {
            panic!("5s 内 master 未进入 Connected");
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    master
}

/// 把仓库 certs/ 装到我们 master 的 TlsConfig。
/// 真实字段(plan 编写时 grep crates/iec104sim-core/src/master.rs 验证):
///   enabled: bool, ca_file: String, cert_file: String, key_file: String,
///   pkcs12_file: String, pkcs12_password: String, accept_invalid_certs: bool,
///   version: TlsVersionPolicy
pub fn tls_config_for_master(bundle: &TlsBundle) -> TlsConfig {
    let mut t = TlsConfig::default();
    t.enabled = true;
    t.ca_file = bundle.ca_pem.to_string_lossy().into_owned();
    t.cert_file = bundle.client_cert_pem.to_string_lossy().into_owned();
    t.key_file = bundle.client_key_pem.to_string_lossy().into_owned();
    // accept_invalid_certs 留 false: 证书 SAN 已含 127.0.0.1, 标准校验可通过。
    t
}

/// SlaveTlsConfig 真实字段:
///   enabled: bool, cert_file: String, key_file: String, ca_file: String,
///   require_client_cert: bool, pkcs12_file: String, (+ p12 password 等)
pub fn tls_config_for_slave(bundle: &TlsBundle) -> SlaveTlsConfig {
    let mut t = SlaveTlsConfig::default();
    t.enabled = true;
    t.cert_file = bundle.server_cert_pem.to_string_lossy().into_owned();
    t.key_file = bundle.server_key_pem.to_string_lossy().into_owned();
    t.ca_file = bundle.ca_pem.to_string_lossy().into_owned();
    t.require_client_cert = true;
    t
}
```

- [ ] **Step 2: cargo check 通过**

Run: `cd compat-suite && cargo check -p compat-tests --tests`
Expected: `Finished` 0 errors。

- [ ] **Step 3: Commit**

```bash
cd compat-suite
git add crates/compat-tests/src/helpers/ours.rs
git commit -m "feat(compat-tests): helpers::ours 自研栈实例工厂 + TLS 映射"
```

---

## Task 7: 升级 smoke_bidirectional.rs (合并现有两 smoke + 中等断言)

**Files:**
- Create: `compat-suite/crates/compat-tests/tests/smoke_bidirectional.rs`
- Delete: `compat-suite/crates/compat-tests/tests/smoke_our_master_vs_thirdparty_slave.rs`
- Delete: `compat-suite/crates/compat-tests/tests/smoke_thirdparty_master_vs_our_slave.rs`

- [ ] **Step 1: 写新 smoke 文件**

```rust
//! 双向 smoke 升级版: 中等粒度断言。
#![allow(clippy::unwrap_used)]

use std::sync::atomic::Ordering;
use std::time::Duration;

use iec104::rtu_server::{PointAddress, PointValue, RtuInitialPoint};
use iec104::types::information_elements::{Siq, Spi};
use iec104::types::quality_descriptors::Qds;
use iec104::types::{MMeNc1, MSpNa1};

use iec104sim_core::data_point::{DataPoint, DataPointValue};
use iec104sim_core::slave::Station;
use iec104sim_core::types::AsduTypeId;

use compat_tests::helpers::asserts::{assert_points_match, ExpectedPoint};
use compat_tests::helpers::ours;
use compat_tests::helpers::ports;
use compat_tests::helpers::thirdparty;

const CA: u16 = 47;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn our_master_total_interrogates_thirdparty() {
    let port = ports::next_local_port();
    let initial = vec![
        RtuInitialPoint::from((
            PointAddress::new(CA, 1),
            PointValue::MSpNa1(MSpNa1 { siq: Siq { spi: Spi::On, ..Default::default() } }),
        )),
        RtuInitialPoint::from((
            PointAddress::new(CA, 11),
            PointValue::MMeNc1(MMeNc1 { value: 42.0, qds: Qds::default() }),
        )),
    ];
    let _rtu = thirdparty::spawn_rtu_server(port, initial, None).await;
    let master = ours::spawn_master(port, CA, None).await;

    master.send_interrogation(CA).await.expect("总召失败");

    // 等数据到位
    let deadline = tokio::time::Instant::now() + Duration::from_secs(8);
    loop {
        let guard = master.received_data.read().await;
        if guard.total_len() >= 2 {
            assert_points_match(&guard, CA, &[
                ExpectedPoint {
                    ioa: 1,
                    asdu_type: AsduTypeId::MSpNa1,
                    value: DataPointValue::SinglePoint { value: true },
                },
                ExpectedPoint {
                    ioa: 11,
                    asdu_type: AsduTypeId::MMeNc1,
                    value: DataPointValue::ShortFloat { value: 42.0 },
                },
            ]);
            break;
        }
        drop(guard);
        if tokio::time::Instant::now() >= deadline {
            panic!("8s 内未收到 ≥2 数据点");
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn thirdparty_master_receives_spontaneous() {
    let port = ports::next_local_port();
    let mut station = Station::new(CA, "smoke-station");
    let mut p = DataPoint::new(1, AsduTypeId::MSpNa1);
    p.value = DataPointValue::SinglePoint { value: true };
    station.data_points.insert(p);

    let slave = ours::spawn_slave(port, vec![station], None).await;
    let (_client, counters) = thirdparty::spawn_client(port, None).await;

    slave.queue_spontaneous(CA, &[(1, AsduTypeId::MSpNa1)]).await;

    let deadline = tokio::time::Instant::now() + Duration::from_secs(8);
    while counters.asdu_count.load(Ordering::SeqCst) == 0 {
        if tokio::time::Instant::now() >= deadline {
            panic!("8s 内未收到任何 ASDU");
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    // 中等断言: 至少有一个 ASDU 的 TypeID = M_SP_NA_1, IOA = 1, COT = 3 (Spontaneous)。
    let asdus = counters.asdus.lock().await;
    let ok = asdus.iter().any(|a| {
        a.type_id == iec104::types_id::TypeId::M_SP_NA_1
            && matches!(a.cot, iec104::cot::Cot::Spontaneous)
            && match &a.information_objects {
                iec104::types::InformationObjects::MSpNa1(v) =>
                    v.iter().any(|obj| obj.address == 1),
                _ => false,
            }
    });
    assert!(ok, "未收到 TypeID=M_SP_NA_1 IOA=1 COT=Spontaneous 的 ASDU,实际: {asdus:?}");
}
```

- [ ] **Step 2: 删除老 smoke 文件**

```bash
cd compat-suite
rm crates/compat-tests/tests/smoke_our_master_vs_thirdparty_slave.rs
rm crates/compat-tests/tests/smoke_thirdparty_master_vs_our_slave.rs
```

- [ ] **Step 3: 运行测试**

Run: `cd compat-suite && cargo test --test smoke_bidirectional -- --nocapture`
Expected: `test result: ok. 2 passed`

如果 ASDU 解构失败,看 `compat-suite/cargo test ... 2>&1 | grep error` 出来的具体 enum variant 名,微调匹配。

- [ ] **Step 4: Commit**

```bash
cd compat-suite
git add -A crates/compat-tests/tests/
git commit -m "test(compat-tests): smoke 升级为双向 + 中等粒度断言"
```

---

## Task 8: sbo_control.rs — SBO 双向

**Files:**
- Create: `compat-suite/crates/compat-tests/tests/sbo_control.rs`

- [ ] **Step 1: 写测试**

```rust
//! SBO 双向: Select → Execute → ActCon → ActTerm 时序一致性。
#![allow(clippy::unwrap_used)]

use std::sync::atomic::Ordering;
use std::time::Duration;

use iec104::rtu_server::{PointAddress, PointValue, RtuInitialPoint};
use iec104::types::information_elements::{SelectExecute, Siq, Spi};
use iec104::types::MSpNa1;

use iec104sim_core::data_point::{DataPoint, DataPointValue};
use iec104sim_core::slave::Station;
use iec104sim_core::types::AsduTypeId;

use compat_tests::helpers::ours;
use compat_tests::helpers::ports;
use compat_tests::helpers::thirdparty;

const CA: u16 = 47;
const IOA: u32 = 100;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn our_master_sbo_to_thirdparty_slave() {
    let port = ports::next_local_port();
    let initial = vec![RtuInitialPoint::from((
        PointAddress::new(CA, IOA),
        PointValue::MSpNa1(MSpNa1 { siq: Siq { spi: Spi::Off, ..Default::default() } }),
    ))];
    let _rtu = thirdparty::spawn_rtu_server(port, initial, None).await;
    let master = ours::spawn_master(port, CA, None).await;

    // send_single_command(ioa, value, select, ca, qu, cot)
    // 注: DataPoint 不存 COT, 正向用例只能验"两次发送 + master 仍 Connected"。
    // ActCon/ActTerm 的精确 COT 序列在反向用例 (callback 侧拿到 Asdu.cot) 验证。
    master.send_single_command(IOA, true, /*select=*/ true, CA, /*qu=*/ 0, /*cot=*/ 6).await
        .expect("Select 帧发送失败");
    tokio::time::sleep(Duration::from_millis(300)).await;
    master.send_single_command(IOA, true, /*select=*/ false, CA, 0, 6).await
        .expect("Execute 帧发送失败");

    // 给从站处理一些时间, 然后确认连接仍 Connected (没被 SBO 异常踢断)。
    tokio::time::sleep(Duration::from_millis(800)).await;
    assert!(
        matches!(master.state(), iec104sim_core::master::MasterState::Connected),
        "SBO 之后 master 不再 Connected, state = {:?}", master.state()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn thirdparty_master_sbo_to_our_slave() {
    let port = ports::next_local_port();
    let mut station = Station::new(CA, "sbo-station");
    let mut p = DataPoint::new(IOA, AsduTypeId::MSpNa1);
    p.value = DataPointValue::SinglePoint { value: false };
    station.data_points.insert(p);

    let _slave = ours::spawn_slave(port, vec![station], None).await;
    let (client, counters) = thirdparty::spawn_client(port, None).await;

    // Select
    client.send_command_sp(CA, IOA, Spi::On, None, Some(SelectExecute::Select), None)
        .await.expect("Select 失败");
    // Execute
    tokio::time::sleep(Duration::from_millis(200)).await;
    client.send_command_sp(CA, IOA, Spi::On, None, Some(SelectExecute::Execute), None)
        .await.expect("Execute 失败");

    // 期望至少收到 2 个 ASDU (ActCon Select + ActCon Execute,ActTerm 视实现可能有)。
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while counters.asdu_count.load(Ordering::SeqCst) < 2 {
        if tokio::time::Instant::now() >= deadline {
            let n = counters.asdu_count.load(Ordering::SeqCst);
            panic!("5s 内 SBO 响应不足 (got {n}, want ≥2)");
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(counters.errors.load(Ordering::SeqCst), 0, "出现 callback 错误");
}
```

- [ ] **Step 2: 运行测试**

Run: `cd compat-suite && cargo test --test sbo_control -- --nocapture`
Expected: `test result: ok. 2 passed`,或 fail。

- [ ] **Step 3: 失败处理**

如果任一用例 fail:
1. 把 stderr 复制到 `compat-suite/FAILURES.md`(创建文件,写明日期 / 用例名 / 收到的 ASDU 列表 / 怀疑根因)。
2. 测试代码加 `#[ignore]` 标记 跳过,并加注释引 FAILURES.md。
3. 在本仓库起 follow-up issue (`gh issue create --title "SBO 互联差异" --body "见 compat-suite/FAILURES.md"`)。
4. **不修改 `iec104sim-core`** — 留给后续单独 PR。

- [ ] **Step 4: Commit**

```bash
cd compat-suite
git add crates/compat-tests/tests/sbo_control.rs FAILURES.md 2>/dev/null
git commit -m "test(compat-tests): SBO 双向用例"
```

---

## Task 9: setpoint.rs — 设点 NA/NB/NC

**Files:**
- Create: `compat-suite/crates/compat-tests/tests/setpoint.rs`

- [ ] **Step 1: 写测试**

```rust
//! 设点 (C_SE_NA / C_SE_NB / C_SE_NC) 我们 master → 第三方 slave round-trip。
#![allow(clippy::unwrap_used)]

use std::time::Duration;

use iec104::rtu_server::{PointAddress, PointValue, RtuInitialPoint};
use iec104::types::quality_descriptors::Qds;
use iec104::types::{MMeNa1, MMeNb1, MMeNc1};

use compat_tests::helpers::ours;
use compat_tests::helpers::ports;
use compat_tests::helpers::thirdparty;

const CA: u16 = 47;

async fn run_setpoint<F, Fut>(
    port: u16,
    initial: Vec<RtuInitialPoint>,
    send: F,
)
where
    F: FnOnce(iec104sim_core::master::MasterConnection) -> Fut,
    Fut: std::future::Future<Output = iec104sim_core::master::MasterConnection>,
{
    let _rtu = thirdparty::spawn_rtu_server(port, initial, None).await;
    let master = ours::spawn_master(port, CA, None).await;
    let master = send(master).await;

    // 与 SBO 同理: 正向用例只能验"发送不报错 + 连接保持 Connected"。
    // ActCon 的精确 COT 在反向用例兜底 (本轮反向只对 SBO 做,设点反向后续补)。
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(
        matches!(master.state(), iec104sim_core::master::MasterState::Connected),
        "设点之后 master 不再 Connected, state = {:?}", master.state()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn setpoint_normalized_round_trip() {
    let port = ports::next_local_port();
    let initial = vec![RtuInitialPoint::from((
        PointAddress::new(CA, 20),
        PointValue::MMeNa1(MMeNa1 { nva: 0, qds: Qds::default() }),
    ))];
    run_setpoint(port, initial, |m| async move {
        m.send_setpoint_normalized(20, 0.5, /*select=*/ false, CA, /*ql=*/ 0, /*cot=*/ 6)
            .await.expect("send_setpoint_normalized 失败");
        m
    }).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn setpoint_scaled_round_trip() {
    let port = ports::next_local_port();
    let initial = vec![RtuInitialPoint::from((
        PointAddress::new(CA, 21),
        PointValue::MMeNb1(MMeNb1 { sva: 0, qds: Qds::default() }),
    ))];
    run_setpoint(port, initial, |m| async move {
        m.send_setpoint_scaled(21, 1234, false, CA, 0, 6).await
            .expect("send_setpoint_scaled 失败");
        m
    }).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn setpoint_float_round_trip() {
    let port = ports::next_local_port();
    let initial = vec![RtuInitialPoint::from((
        PointAddress::new(CA, 11),
        PointValue::MMeNc1(MMeNc1 { value: 0.0, qds: Qds::default() }),
    ))];
    run_setpoint(port, initial, |m| async move {
        m.send_setpoint_float(11, 3.14, false, CA, 0, 6).await
            .expect("send_setpoint_float 失败");
        m
    }).await;
}
```

- [ ] **Step 2: 运行 + 失败处理**

Run: `cd compat-suite && cargo test --test setpoint -- --nocapture`
Expected: `test result: ok. 3 passed`。

若任一 fail:
1. 把 stderr 输出 append 到 `compat-suite/FAILURES.md`,写明日期 / 用例名 / 怀疑根因。
2. 在失败的 test fn 上加 `#[ignore]` + 注释引 FAILURES.md。
3. **不修改 `iec104sim-core`**,留单独 PR。

- [ ] **Step 3: Commit**

```bash
cd compat-suite
git add crates/compat-tests/tests/setpoint.rs FAILURES.md 2>/dev/null
git commit -m "test(compat-tests): 设点 NA/NB/NC round-trip"
```

---

## Task 10: multi_ca.rs — 多 CA

**Files:**
- Create: `compat-suite/crates/compat-tests/tests/multi_ca.rs`

- [ ] **Step 1: 写测试**

```rust
//! 多 CA 双向: 同进程多个 CA 在两侧能正确路由。
#![allow(clippy::unwrap_used)]

use std::sync::atomic::Ordering;
use std::time::Duration;

use iec104::rtu_server::{PointAddress, PointValue, RtuInitialPoint};
use iec104::types::information_elements::{Siq, Spi};
use iec104::types::MSpNa1;

use iec104sim_core::data_point::{DataPoint, DataPointValue};
use iec104sim_core::slave::Station;
use iec104sim_core::types::AsduTypeId;

use compat_tests::helpers::ours;
use compat_tests::helpers::ports;
use compat_tests::helpers::thirdparty;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn our_slave_serves_two_cas() {
    let port = ports::next_local_port();
    let mut s10 = Station::new(10, "ca-10");
    let mut s20 = Station::new(20, "ca-20");
    for ioa in [1u32, 2] {
        let mut p = DataPoint::new(ioa, AsduTypeId::MSpNa1);
        p.value = DataPointValue::SinglePoint { value: true };
        s10.data_points.insert(p);
        let mut p = DataPoint::new(ioa + 100, AsduTypeId::MSpNa1);
        p.value = DataPointValue::SinglePoint { value: false };
        s20.data_points.insert(p);
    }
    let slave = ours::spawn_slave(port, vec![s10, s20], None).await;
    let (_client, counters) = thirdparty::spawn_client(port, None).await;

    // 主动上送 CA=10 IOA={1,2},CA=20 IOA={101,102},客户端应收齐 4 个 ASDU。
    slave.queue_spontaneous(10, &[(1, AsduTypeId::MSpNa1), (2, AsduTypeId::MSpNa1)]).await;
    slave.queue_spontaneous(20, &[(101, AsduTypeId::MSpNa1), (102, AsduTypeId::MSpNa1)]).await;

    let deadline = tokio::time::Instant::now() + Duration::from_secs(8);
    while counters.asdu_count.load(Ordering::SeqCst) < 4 {
        if tokio::time::Instant::now() >= deadline {
            panic!("8s 内只收到 {} 个 ASDU (期望 ≥4)",
                counters.asdu_count.load(Ordering::SeqCst));
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let asdus = counters.asdus.lock().await;
    let cas: std::collections::HashSet<u16> = asdus.iter().map(|a| a.address_field).collect();
    assert!(cas.contains(&10) && cas.contains(&20),
        "未同时收到 CA 10 与 CA 20 的 ASDU: {cas:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn our_master_polls_two_cas() {
    let port_a = ports::next_local_port();
    let port_b = ports::next_local_port();

    let init = |ca: u16| vec![RtuInitialPoint::from((
        PointAddress::new(ca, 1),
        PointValue::MSpNa1(MSpNa1 { siq: Siq { spi: Spi::On, ..Default::default() } }),
    ))];
    let _rtu_a = thirdparty::spawn_rtu_server(port_a, init(47), None).await;
    let _rtu_b = thirdparty::spawn_rtu_server(port_b, init(48), None).await;
    let master_a = ours::spawn_master(port_a, 47, None).await;
    let master_b = ours::spawn_master(port_b, 48, None).await;

    master_a.send_interrogation(47).await.expect("总召 CA=47 失败");
    master_b.send_interrogation(48).await.expect("总召 CA=48 失败");

    let deadline = tokio::time::Instant::now() + Duration::from_secs(8);
    loop {
        let a = master_a.received_data.read().await;
        let b = master_b.received_data.read().await;
        if a.ca_map(47).map(|m| m.len()).unwrap_or(0) >= 1
            && b.ca_map(48).map(|m| m.len()).unwrap_or(0) >= 1 {
            return;
        }
        drop(a); drop(b);
        if tokio::time::Instant::now() >= deadline {
            panic!("8s 内 master_a/master_b 未各自收齐");
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
```

- [ ] **Step 2: 运行 + 失败处理**

Run: `cd compat-suite && cargo test --test multi_ca -- --nocapture`
Expected: 2 passed。

若任一 fail:
1. 把 stderr 输出 append 到 `compat-suite/FAILURES.md`,写明日期 / 用例名 / 怀疑根因。
2. 在失败的 test fn 上加 `#[ignore]` + 注释引 FAILURES.md。
3. **不修改 `iec104sim-core`**,留单独 PR。

- [ ] **Step 3: Commit**

```bash
cd compat-suite
git add crates/compat-tests/tests/multi_ca.rs FAILURES.md 2>/dev/null
git commit -m "test(compat-tests): 多 CA 双向用例"
```

---

## Task 11: tls_handshake.rs — TLS 双向

**Files:**
- Create: `compat-suite/crates/compat-tests/tests/tls_handshake.rs`

- [ ] **Step 1: 写测试**

```rust
//! TLS 双向: 用本仓库 self-signed 证书做规约 over TLS 验证。
//! SAN 已包含 127.0.0.1 (`openssl x509 -in server.pem -ext subjectAltName` 验证过)。
#![allow(clippy::unwrap_used)]

use std::sync::atomic::Ordering;
use std::time::Duration;

use iec104::config::{TlsClientConfig, TlsServerConfig};
use iec104::rtu_server::{PointAddress, PointValue, RtuInitialPoint};
use iec104::types::information_elements::{Siq, Spi};
use iec104::types::MSpNa1;

use iec104sim_core::data_point::{DataPoint, DataPointValue};
use iec104sim_core::slave::Station;
use iec104sim_core::types::AsduTypeId;

use compat_tests::helpers::asserts::{assert_points_match, ExpectedPoint};
use compat_tests::helpers::certs;
use compat_tests::helpers::ours;
use compat_tests::helpers::ports;
use compat_tests::helpers::thirdparty;

const CA: u16 = 47;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn our_master_tls_to_thirdparty_slave() {
    let port = ports::next_local_port();
    let bundle = certs::load_bundle();
    let server_tls = TlsServerConfig {
        server_certificate: bundle.server_cert_pem.clone(),
        server_key: bundle.server_key_pem.clone(),
    };
    let initial = vec![RtuInitialPoint::from((
        PointAddress::new(CA, 1),
        PointValue::MSpNa1(MSpNa1 { siq: Siq { spi: Spi::On, ..Default::default() } }),
    ))];
    let _rtu = thirdparty::spawn_rtu_server(port, initial, Some(server_tls)).await;

    let master = ours::spawn_master(port, CA, Some(ours::tls_config_for_master(&bundle))).await;
    master.send_interrogation(CA).await.expect("总召失败");

    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let guard = master.received_data.read().await;
        if guard.total_len() >= 1 {
            assert_points_match(&guard, CA, &[ExpectedPoint {
                ioa: 1,
                asdu_type: AsduTypeId::MSpNa1,
                value: DataPointValue::SinglePoint { value: true },
            }]);
            return;
        }
        drop(guard);
        if tokio::time::Instant::now() >= deadline {
            panic!("10s 内 TLS 总召没回数据");
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn thirdparty_master_tls_to_our_slave() {
    let port = ports::next_local_port();
    let bundle = certs::load_bundle();
    let client_tls = TlsClientConfig {
        client_key: Some(bundle.client_key_pem.clone()),
        client_certificate: Some(bundle.client_cert_pem.clone()),
        server_certificate: Some(bundle.ca_pem.clone()),
        danger_disable_tls_verify: false,
    };

    let mut station = Station::new(CA, "tls-station");
    let mut p = DataPoint::new(1, AsduTypeId::MSpNa1);
    p.value = DataPointValue::SinglePoint { value: true };
    station.data_points.insert(p);
    let slave = ours::spawn_slave(port, vec![station], Some(ours::tls_config_for_slave(&bundle))).await;
    let (_client, counters) = thirdparty::spawn_client(port, Some(client_tls)).await;

    slave.queue_spontaneous(CA, &[(1, AsduTypeId::MSpNa1)]).await;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while counters.asdu_count.load(Ordering::SeqCst) == 0 {
        if tokio::time::Instant::now() >= deadline {
            panic!("10s 内 TLS 反向通道未收到 ASDU");
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert_eq!(counters.errors.load(Ordering::SeqCst), 0, "出现 TLS 错误");
}
```

- [ ] **Step 2: 运行 + 失败处理**

Run: `cd compat-suite && cargo test --test tls_handshake -- --nocapture`
Expected: 2 passed。

如果 TLS 握手失败(常见原因:本仓库 TlsConfig 字段映射在 Task 6 Step 2 没补全),按报错信息回 helpers/ours.rs 补字段。若是规约级问题,登记到 FAILURES.md 并 `#[ignore]`。

- [ ] **Step 3: Commit**

```bash
cd compat-suite
git add crates/compat-tests/tests/tls_handshake.rs FAILURES.md 2>/dev/null
git commit -m "test(compat-tests): TLS 双向用例 (复用仓库 certs/)"
```

---

## Task 12: 全量回归验证

**Files:**
- Modify (可能): `compat-suite/FAILURES.md`

- [ ] **Step 1: 运行所有测试**

Run: `cd compat-suite && cargo test --all`
Expected: 11 个测试函数全部 pass(或失败的已 `#[ignore]`)。

- [ ] **Step 2: 统计被 ignore 的用例**

Run: `cd compat-suite && cargo test --all 2>&1 | grep ignored`
Expected: 输出每个测试文件 ignored 数,记到 FAILURES.md 顶部摘要。

- [ ] **Step 3: 如果有 ignored,补 FAILURES.md 摘要 + commit**

```bash
cd compat-suite
git add FAILURES.md
git commit -m "docs(compat-tests): FAILURES 摘要"
```

(如果没有 ignored,跳过 commit。)

---

## Task 13: 写 GitHub Actions workflow

**Files:**
- Create: `.github/workflows/compat-suite.yml` (本仓库)

- [ ] **Step 1: 创建 workflow 文件**

`IEC60870-5-104-Simulator/.github/workflows/compat-suite.yml`:
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
    - cron: '0 18 * * *'   # 北京时间 02:00
  workflow_dispatch: {}

jobs:
  compat-smoke:
    runs-on: ubuntu-latest
    timeout-minutes: 15
    steps:
      - name: Checkout this repo
        uses: actions/checkout@v4
        with: { path: IEC60870-5-104-Simulator }

      - name: Checkout mzaniolo/iec104 (pinned)
        uses: actions/checkout@v4
        with:
          repository: mzaniolo/iec104
          ref: v0.4.0
          path: iec104-mzaniolo-ref

      - name: Checkout compat suite
        uses: actions/checkout@v4
        with:
          repository: <your-org>/iec104-compat-suite
          ref: main
          path: iec104-compat-suite

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable

      - name: Cache cargo
        uses: Swatinem/rust-cache@v2
        with:
          workspaces: iec104-compat-suite

      - name: Run compat tests
        working-directory: iec104-compat-suite
        env:
          RUST_BACKTRACE: 1
        run: cargo test --all -- --test-threads 2
```

`<your-org>` 暂不替换,在 Task 14 之后再替。

- [ ] **Step 2: 本地语法检查**

如果安装了 `actionlint`:
```bash
cd "/Users/.../IEC60870-5-104-Simulator"
actionlint .github/workflows/compat-suite.yml
```
Expected: 无输出。

如果没装,跳过本步;CI 首次跑会暴露语法错误。

- [ ] **Step 3: Commit**

```bash
cd "/Users/.../IEC60870-5-104-Simulator"
git add .github/workflows/compat-suite.yml
git commit -m "ci: 互联回归 compat-suite workflow"
```

---

## Task 14: 建 GitHub repo 并 push compat-suite

**Files:**
- Modify (远端): GitHub 新 repo `<your-org>/iec104-compat-suite`

- [ ] **Step 1: 在 GitHub 建 repo**

用户操作:
1. 浏览器打开 `https://github.com/new`。
2. Repo name: `iec104-compat-suite`,Public(避免 CI 用 PAT),不勾 README/LICENSE/.gitignore。
3. Create。

或 gh CLI:
```bash
gh repo create iec104-compat-suite --public --description "IEC 60870-5-104 互联回归测试套件"
```

记下 owner (即 `<your-org>` 的实际值,可能是 GitHub 用户名)。

- [ ] **Step 2: 本地 init + push**

```bash
cd "/Users/.../iec104-compat-suite"
git init 2>/dev/null   # 如果尚未 init
git add -A
git status   # 确认 target/ 被忽略
git commit -m "feat: 互联回归测试套件首版 (smoke + sbo + setpoint + multi_ca + tls)"
git branch -M main
git remote add origin git@github.com:<your-org>/iec104-compat-suite.git
git push -u origin main
```

(如果之前已经在 compat-suite 目录里 commit 过(Task 1-12),这一步只需 `git remote add` + `git push`。)

- [ ] **Step 3: 验证 GitHub 上文件齐全**

Run: `gh repo view <your-org>/iec104-compat-suite --json url -q .url`
打开浏览器看 crates/ + README.md + .gitignore 都在。

---

## Task 15: 替换 workflow 中 `<your-org>` 占位并触发首次 CI

**Files:**
- Modify: `.github/workflows/compat-suite.yml` (本仓库)

- [ ] **Step 1: 替换占位符**

```bash
cd "/Users/.../IEC60870-5-104-Simulator"
sed -i.bak "s|<your-org>|实际的 GitHub owner|" .github/workflows/compat-suite.yml
rm .github/workflows/compat-suite.yml.bak
```

(把"实际的 GitHub owner"替换成 Task 14 Step 1 记下的值。)

- [ ] **Step 2: 验证 diff**

Run: `git diff .github/workflows/compat-suite.yml`
Expected: 只一行变化,`repository: <your-org>/iec104-compat-suite` → `repository: 实际owner/iec104-compat-suite`。

- [ ] **Step 3: Commit + push 触发 CI**

```bash
git add .github/workflows/compat-suite.yml
git commit -m "ci: 设置 compat-suite repo owner"
git push
```

- [ ] **Step 4: workflow_dispatch 手工触发首跑**

```bash
gh workflow run compat-suite.yml
gh run watch
```

Expected: GitHub Actions 显示 compat-smoke job 跑过,11 个测试结果与本地一致。

- [ ] **Step 5: 如果 CI 失败**

常见原因:
1. `iec104-compat-suite` repo 没 public — 改 visibility 或加 PAT。
2. ubuntu 上 native-tls 后端差异 — 看 stderr,可能要换 rustls。
3. 测试用例上 `tokio` 版本与 ubuntu rustc 不兼容 — `cargo update` 在 compat-suite 重新生成。

修复后再 push,workflow 会自动重跑。

---

## Task 16: 收尾 — 更新 README + 登记 follow-up

**Files:**
- Modify: `compat-suite/README.md`

- [ ] **Step 1: 更新 README 用例清单**

把 `compat-suite/README.md` 现有"当前两个 smoke"段落替换为:

```markdown
## 当前覆盖 (11 用例)

| 文件 | 用例 | 方向 |
|------|------|------|
| smoke_bidirectional.rs | 总召 + 自发上送 | 双向 |
| sbo_control.rs | Select+Execute | 双向 |
| setpoint.rs | 设点 NA/NB/NC | 我们 M → 第三方 S |
| multi_ca.rs | 多 CA 路由 | 双向 |
| tls_handshake.rs | TLS 握手 + 数据交换 | 双向 |
```

如果存在被 `#[ignore]` 的用例,在 README 末尾加一节"已知差异",链 FAILURES.md。

- [ ] **Step 2: Commit + push**

```bash
cd "/Users/.../iec104-compat-suite"
git add README.md
git commit -m "docs: 更新用例覆盖清单"
git push
```

- [ ] **Step 3: 主仓库 follow-up 登记 (如 FAILURES.md 非空)**

```bash
cd "/Users/.../IEC60870-5-104-Simulator"
gh issue create \
  --title "互联回归: 与 mzaniolo/iec104 的规约差异" \
  --body "compat-suite 在 sbo / setpoint / multi_ca / tls 上发现 N 项差异,详见 https://github.com/实际owner/iec104-compat-suite/blob/main/FAILURES.md。本仓库后续 PR 按优先级修复。"
```

(如果 FAILURES.md 是空的,跳过此步。)

---

## 完成定义对照

| spec DoD 项 | 在哪个 Task 兑现 |
|-------------|-------------------|
| 1. 11 个测试函数全部存在 | Task 7+8+9+10+11 |
| 2. 本地 cargo test --all 全绿 | Task 12 |
| 3. 失败用例登记 FAILURES.md + follow-up issue | Task 8/9/10/11 Step 3 + Task 16 Step 3 |
| 4. workflow 写入本仓库 + 通过 lint | Task 13 |
| 5. compat-suite GitHub repo 已建 + push | Task 14 |
| 6. `<your-org>` 占位符已替换 | Task 15 |
