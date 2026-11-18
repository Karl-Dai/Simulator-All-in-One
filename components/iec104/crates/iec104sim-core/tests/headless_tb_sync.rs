//! slave-na-default-tb-derived 变更回归:
//! - 按分类的「变位同步上送 TB」开关(5.2)与 IT 不提供开关(5.5)
//! - R1 显式 TB 压制派生:变位路径(5.3)与总召唤路径(5.4)

mod common;
use common::harness::Pair;
use common::helpers::{count_iframes, wait_for_ioa_count, DEFAULT_TIMEOUT};

use iec104sim_core::data_point::{DataPointValue, InformationObjectDef};
use iec104sim_core::log_entry::Direction;
use iec104sim_core::slave::{RemoteOperationConfig, SyncTbByCategory};
use iec104sim_core::types::{AsduTypeId, DataCategory};
use tokio::time::{sleep, Duration};

/// GI 后等数据稳定再清日志,排除 GI 应答噪声。
async fn gi_then_clear(pair: &Pair) {
    pair.master.conn.send_interrogation(1).await.unwrap();
    let _ = wait_for_ioa_count(&pair.master.conn, 1, 1, DEFAULT_TIMEOUT).await;
    sleep(Duration::from_millis(400)).await;
    pair.log.clear().await;
}

/// 显式给某 IOA 加一个 TB 点(R1 测试用)。
async fn add_explicit_point(pair: &Pair, ioa: u32, asdu: AsduTypeId, cat: DataCategory) {
    let mut stations = pair.slave.server.stations.write().await;
    let st = stations.get_mut(&1).unwrap();
    st.add_point(InformationObjectDef {
        ioa,
        asdu_type: asdu,
        category: cat,
        name: String::new(),
        comment: String::new(), mapping: None,
        command_qualifier: None, select_before_operate: None
    })
    .unwrap();
}

async fn mutate_and_queue(pair: &Pair, changes: &[(u32, AsduTypeId, DataPointValue)]) {
    {
        let mut stations = pair.slave.server.stations.write().await;
        let st = stations.get_mut(&1).unwrap();
        for (ioa, asdu, val) in changes {
            if let Some(p) = st.data_points.get_mut(*ioa, *asdu) {
                p.value = val.clone();
            }
            st.data_points.mark_changed(*ioa, *asdu);
        }
    }
    let ids: Vec<(u32, AsduTypeId)> = changes.iter().map(|(i, a, _)| (*i, *a)).collect();
    pair.slave.server.queue_spontaneous(1, &ids).await;
    sleep(Duration::from_millis(500)).await;
}

/// 5.2 + 5.5:仅 SP 开关开 → 变位时 SP 派生 TB,DP(关)与 IT(无开关)不派生。
#[tokio::test]
async fn per_category_gating_and_it_excluded() {
    let ops = RemoteOperationConfig {
        sync_tb_by_category: SyncTbByCategory { sp: true, ..Default::default() },
        ..Default::default()
    };
    let pair = Pair::spawn_with(ops, 1).await;
    gi_then_clear(&pair).await;

    mutate_and_queue(&pair, &[
        (1, AsduTypeId::MSpNa1, DataPointValue::SinglePoint { value: true }),
        (1, AsduTypeId::MDpNa1, DataPointValue::DoublePoint { value: 2 }),
        (1, AsduTypeId::MItNa1, DataPointValue::IntegratedTotal { value: 100, carry: false, sequence: 0 }),
    ]).await;

    let sp_tb = count_iframes(&pair.log, Direction::Rx, "M_SP_TB_1").await;
    let dp_tb = count_iframes(&pair.log, Direction::Rx, "M_DP_TB_1").await;
    let it_tb = count_iframes(&pair.log, Direction::Rx, "M_IT_TB_1").await;
    assert!(sp_tb >= 1, "SP 开关开,应派生 M_SP_TB_1, got {}", sp_tb);
    assert_eq!(dp_tb, 0, "DP 开关关,不应派生 M_DP_TB_1");
    assert_eq!(it_tb, 0, "IT 无同步开关,不应派生 M_IT_TB_1");

    pair.shutdown().await;
}

/// 5.3:某 IOA 已有显式 TB 点 → SP 开关开,NA 变位也不再派生 TB(R1,变位路径)。
#[tokio::test]
async fn r1_explicit_tb_suppresses_spontaneous_derive() {
    let ops = RemoteOperationConfig {
        sync_tb_by_category: SyncTbByCategory { sp: true, ..Default::default() },
        ..Default::default()
    };
    let pair = Pair::spawn_with(ops, 1).await;
    add_explicit_point(&pair, 1, AsduTypeId::MSpTb1, DataCategory::SinglePoint).await;
    gi_then_clear(&pair).await;

    // 只把 NA 标记变位上送(显式 TB 未变,不会自发)。
    mutate_and_queue(&pair, &[
        (1, AsduTypeId::MSpNa1, DataPointValue::SinglePoint { value: true }),
    ]).await;

    let sp_tb = count_iframes(&pair.log, Direction::Rx, "M_SP_TB_1").await;
    assert_eq!(sp_tb, 0, "IOA=1 已有显式 TB,NA 变位 MUST 不派生 TB(R1)");

    pair.shutdown().await;
}

/// 5.4:gi_include_timestamped + 显式 TB → 总召唤对同一 IOA 只发一份 TB(R1,GI 路径)。
#[tokio::test]
async fn r1_explicit_tb_no_double_on_gi() {
    let ops = RemoteOperationConfig { gi_include_timestamped: true, ..Default::default() };
    let pair = Pair::spawn_with(ops, 1).await;
    add_explicit_point(&pair, 1, AsduTypeId::MSpTb1, DataCategory::SinglePoint).await;

    pair.master.conn.send_interrogation(1).await.unwrap();
    sleep(Duration::from_millis(800)).await;

    // 显式 TB 发一份;NA 的派生被 R1 跳过 → 恰好 1 份,而非 2 份。
    let sp_tb = count_iframes(&pair.log, Direction::Rx, "M_SP_TB_1").await;
    assert_eq!(sp_tb, 1, "GI: 显式 TB 存在时同一 IOA 应只有 1 份 TB(R1 防重复), got {}", sp_tb);

    pair.shutdown().await;
}
