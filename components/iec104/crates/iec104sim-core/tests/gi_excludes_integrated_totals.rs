//! Bug 复现 / 回归:总召唤 (GI, C_IC_NA_1, Type 100) 不应上送累积量 (M_IT)。
//!
//! 按 IEC 60870-5-101/104,总召唤只召唤过程信息(单点/双点/步位置/位串/测量值),
//! 累积量 (M_IT) 只能由计数量召唤 (C_CI_NA_1, Type 101) 上送。
//! 现状:子站 GI 响应路径未过滤 IntegratedTotal,导致总召唤把累积量也带回。

use iec104sim_core::master::{MasterConfig, MasterConnection};
use iec104sim_core::slave::{SlaveServer, SlaveTransportConfig, Station};
use iec104sim_core::types::AsduTypeId;
use tokio::time::{sleep, Duration};

fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

async fn connect_master(port: u16) -> MasterConnection {
    let mut master = MasterConnection::new(MasterConfig {
        target_address: "127.0.0.1".into(),
        port,
        common_address: 1,
        ..Default::default()
    });
    master.connect().await.unwrap();
    sleep(Duration::from_millis(300)).await;
    master
}

async fn start_slave(port: u16, label: &str) -> SlaveServer {
    let mut slave = SlaveServer::new(SlaveTransportConfig {
        bind_address: "127.0.0.1".into(),
        port,
        ..Default::default()
    });
    slave
        .add_station(Station::with_default_points(1, label, 3))
        .await
        .unwrap();
    slave.start().await.unwrap();
    sleep(Duration::from_millis(300)).await;
    slave
}

/// 仅发总召唤:过程信息应到齐,但累积量 (M_IT) 不应被带回。
#[tokio::test]
async fn gi_must_not_return_integrated_totals() {
    let port = free_port();
    let mut slave = start_slave(port, "gi-it").await;
    let mut master = connect_master(port).await;

    // 仅发总召唤 (不发计数量召唤)。
    master.send_interrogation(1).await.unwrap();
    sleep(Duration::from_millis(1500)).await;

    {
        let data = master.received_data.read().await;
        let map = data.ca_map(1).expect("CA=1 map missing after GI");
        // 过程信息应到齐(抽样:单点 + 浮点测量)。
        assert!(map.get(1, AsduTypeId::MSpNa1).is_some(), "GI 后缺少 SinglePoint");
        assert!(map.get(1, AsduTypeId::MMeNc1).is_some(), "GI 后缺少 FloatMeasured");
        // 累积量不应被总召唤带回 —— 这是被复现的 bug。
        assert!(
            map.get(1, AsduTypeId::MItNa1).is_none(),
            "BUG: 总召唤(C_IC_NA_1)上送了累积量(M_IT) — 累积量应仅由计数量召唤(C_CI_NA_1)上送",
        );
    }

    master.disconnect().await.unwrap();
    slave.stop().await.unwrap();
}

/// 反向保证:计数量召唤仍能拿到累积量。
#[tokio::test]
async fn counter_interrogation_still_returns_integrated_totals() {
    let port = free_port();
    let mut slave = start_slave(port, "ci-it").await;
    let mut master = connect_master(port).await;

    master.send_counter_read(1).await.unwrap();
    sleep(Duration::from_millis(1500)).await;
    {
        let data = master.received_data.read().await;
        let map = data.ca_map(1).expect("CA=1 map missing after CI");
        assert!(
            map.get(1, AsduTypeId::MItNa1).is_some(),
            "计数量召唤后应收到累积量(M_IT)",
        );
    }

    master.disconnect().await.unwrap();
    slave.stop().await.unwrap();
}
