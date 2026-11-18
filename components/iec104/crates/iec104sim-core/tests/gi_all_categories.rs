//! Regression test for fix-slave-data-display task 8.5 / 8.6:
//! 子站 `Station::with_default_points` 必须为全部 8 个物理分类初始化点位。
//! 总召唤 (GI, C_IC_NA_1) 只召唤过程信息,应收到 7 个过程量分类——尤其是
//! 后来补齐的 StepPosition (M_ST_NA_1, Type 5) 与 Bitstring (M_BO_NA_1, Type 7);
//! 累积量 (M_IT) 不在 GI 范围内,仅由计数量召唤 (CI, C_CI_NA_1) 上送(8.6)。

use iec104sim_core::master::{MasterConfig, MasterConnection};
use iec104sim_core::slave::{SlaveServer, SlaveTransportConfig, Station};
use iec104sim_core::types::AsduTypeId;
use tokio::time::{sleep, Duration};

fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

/// with_default_points 为每个 ASDU 类型在 IOA 1..=count 上建点,
/// 取 IOA=1 校验每类 NA 变体的存在性即可代表该分类。
/// GI 应返回的 7 个过程量分类(不含累积量 M_IT)。
const EXPECTED_NA_TYPES: [(AsduTypeId, &str); 7] = [
    (AsduTypeId::MSpNa1, "SinglePoint"),
    (AsduTypeId::MDpNa1, "DoublePoint"),
    (AsduTypeId::MStNa1, "StepPosition"),  // Type 5 — 7.1/7.2 补齐
    (AsduTypeId::MBoNa1, "Bitstring"),     // Type 7 — 7.1/7.2 补齐
    (AsduTypeId::MMeNa1, "NormalizedMeasured"),
    (AsduTypeId::MMeNb1, "ScaledMeasured"),
    (AsduTypeId::MMeNc1, "FloatMeasured"),
];

#[tokio::test]
async fn gi_returns_all_eight_categories() {
    let port = free_port();
    let mut slave = SlaveServer::new(SlaveTransportConfig {
        bind_address: "127.0.0.1".into(),
        port,
        ..Default::default()
    });
    slave
        .add_station(Station::with_default_points(1, "all8", 3))
        .await
        .unwrap();
    slave.start().await.unwrap();
    sleep(Duration::from_millis(300)).await;

    let mut master = MasterConnection::new(MasterConfig {
        target_address: "127.0.0.1".into(),
        port,
        common_address: 1,
        ..Default::default()
    });
    master.connect().await.unwrap();
    sleep(Duration::from_millis(300)).await;

    // 总召唤 — 应带回全部 8 类点位。
    master.send_interrogation(1).await.unwrap();
    sleep(Duration::from_millis(1500)).await;
    {
        let data = master.received_data.read().await;
        let map = data.ca_map(1).expect("CA=1 map missing after GI");
        for (asdu, label) in EXPECTED_NA_TYPES {
            assert!(
                map.get(1, asdu).is_some(),
                "GI 后缺少分类 {label} ({asdu:?}) @IOA=1 — with_default_points 未初始化或 GI arm 未覆盖",
            );
        }
        // 累积量 (M_IT) 不应被总召唤带回 — IEC 60870-5-101/104 规定其仅由
        // 计数量召唤 (C_CI_NA_1) 上送。
        assert!(
            map.get(1, AsduTypeId::MItNa1).is_none(),
            "GI 不应上送累积量 (M_IT) — 应仅由计数量召唤上送",
        );
    }

    // 累计量召唤 — IntegratedTotals 仍应存在 (8.6)。
    master.send_counter_read(1).await.unwrap();
    sleep(Duration::from_millis(1500)).await;
    {
        let data = master.received_data.read().await;
        let map = data.ca_map(1).expect("CA=1 map missing after CI");
        assert!(
            map.get(1, AsduTypeId::MItNa1).is_some(),
            "累计量召唤后 IntegratedTotals @IOA=1 丢失",
        );
    }

    master.disconnect().await.unwrap();
    slave.stop().await.unwrap();
}
