//! 集成测试 Harness:一键起一对 SlaveServer + MasterConnection,共享日志。
//!
//! 目标:把"分配端口 → 启 slave → 连 master → 起 STARTDT → 等待稳定"的样板
//! 压缩为单次 `Pair::spawn(ops)`,后续测试只关心场景特定的差异。

#![allow(dead_code)] // harness 函数被多个测试文件按需调用

use std::sync::Arc;

use iec104sim_core::log_collector::LogCollector;
use iec104sim_core::master::{MasterConfig, MasterConnection, MasterState};
use iec104sim_core::slave::{
    RemoteOperationConfig, SlaveServer, SlaveTransportConfig, Station,
};

use super::helpers::{wait_for_master_connected, DEFAULT_TIMEOUT};

/// 分配一个空闲的 TCP 端口供测试 server 使用。
pub fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

/// 构造 Slave 端的可配参数。所有字段都有默认,链式调用按需覆盖。
pub struct SlaveBuilder {
    pub port: u16,
    pub ca: u16,
    pub points_per_category: u32,
    pub remote_ops: Option<RemoteOperationConfig>,
    pub log: Option<Arc<LogCollector>>,
}

impl Default for SlaveBuilder {
    fn default() -> Self {
        Self {
            port: 0,
            ca: 1,
            points_per_category: 5,
            remote_ops: None,
            log: None,
        }
    }
}

impl SlaveBuilder {
    pub fn with_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }
    pub fn with_ca(mut self, ca: u16) -> Self {
        self.ca = ca;
        self
    }
    pub fn with_points_per_category(mut self, n: u32) -> Self {
        self.points_per_category = n;
        self
    }
    pub fn with_remote_ops(mut self, ops: RemoteOperationConfig) -> Self {
        self.remote_ops = Some(ops);
        self
    }
    pub fn with_log(mut self, log: Arc<LogCollector>) -> Self {
        self.log = Some(log);
        self
    }

    /// 启动 server,返回拥有句柄。`station` 已预填 `points_per_category` 个默认点。
    pub async fn spawn(self) -> SlaveHarness {
        let port = if self.port == 0 { free_port() } else { self.port };
        let transport = SlaveTransportConfig {
            bind_address: "127.0.0.1".to_string(),
            port,
            ..Default::default()
        };
        let log = self.log.unwrap_or_else(|| Arc::new(LogCollector::new()));
        let mut server = SlaveServer::new(transport).with_log_collector(log.clone());
        if let Some(ops) = self.remote_ops {
            server.set_remote_ops(ops).await;
        }
        server
            .add_station(Station::with_default_points(
                self.ca,
                format!("CA{}", self.ca),
                self.points_per_category,
            ))
            .await
            .expect("add_station");
        server.start().await.expect("slave start");
        // 给 listener 一个 tick 时间就绪。集成测试中实测 ~50 ms 足够,但保守 200 ms。
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        SlaveHarness { server, port, ca: self.ca, log }
    }
}

pub struct SlaveHarness {
    pub server: SlaveServer,
    pub port: u16,
    pub ca: u16,
    pub log: Arc<LogCollector>,
}

impl SlaveHarness {
    pub async fn shutdown(mut self) {
        let _ = self.server.stop().await;
    }
}

/// 构造 Master 端。`with_log` 共享 slave 的 LogCollector,使断言能跨主从。
pub struct MasterBuilder {
    pub port: u16,
    pub ca: u16,
    pub log: Option<Arc<LogCollector>>,
}

impl MasterBuilder {
    pub fn new(port: u16, ca: u16) -> Self {
        Self { port, ca, log: None }
    }
    pub fn with_log(mut self, log: Arc<LogCollector>) -> Self {
        self.log = Some(log);
        self
    }

    pub async fn connect(self) -> MasterHarness {
        let config = MasterConfig {
            target_address: "127.0.0.1".to_string(),
            port: self.port,
            common_address: self.ca,
            // 缩短 t1/t2/t3,使测试不必等默认 15 s 超时。
            t0: 5,
            t1: 5,
            t2: 3,
            t3: 10,
            ..Default::default()
        };
        let log = self.log.unwrap_or_else(|| Arc::new(LogCollector::new()));
        let mut conn = MasterConnection::new(config).with_log_collector(log.clone());
        conn.connect().await.expect("master connect");
        // 等到 STARTDT 完成,基于状态机而不是盲 sleep。
        wait_for_master_connected(&conn, DEFAULT_TIMEOUT)
            .await
            .expect("wait connected");
        MasterHarness { conn, log }
    }
}

pub struct MasterHarness {
    pub conn: MasterConnection,
    pub log: Arc<LogCollector>,
}

impl MasterHarness {
    pub fn state(&self) -> MasterState {
        self.conn.state()
    }
    pub async fn shutdown(mut self) {
        let _ = self.conn.disconnect().await;
    }
}

/// 一键起一对主子站,共享同一个 LogCollector。
pub struct Pair {
    pub slave: SlaveHarness,
    pub master: MasterHarness,
    pub log: Arc<LogCollector>,
}

impl Pair {
    pub async fn spawn(ops: RemoteOperationConfig) -> Self {
        Self::spawn_with(ops, 5).await
    }

    pub async fn spawn_with(ops: RemoteOperationConfig, points_per_category: u32) -> Self {
        let log = Arc::new(LogCollector::new());
        let slave = SlaveBuilder::default()
            .with_remote_ops(ops)
            .with_log(log.clone())
            .with_points_per_category(points_per_category)
            .spawn()
            .await;
        let master = MasterBuilder::new(slave.port, slave.ca)
            .with_log(log.clone())
            .connect()
            .await;
        Self { slave, master, log }
    }

    pub async fn shutdown(self) {
        self.master.shutdown().await;
        self.slave.shutdown().await;
    }
}
