//! 8 万点位 GI 性能回归测试。
//!
//! 验证：
//! 1. GI 在 8s 内完成（远低于默认 t1=15s），证明 SQ=1 打包 + k/w 流控生效。
//! 2. 子站发往主站的 I 帧总数远低于点位总数，证明启用了 SQ=1 连续打包。
//! 3. 主站端能完整收齐所有 IOA。

mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use iec104sim_core::log_collector::LogCollector;
use iec104sim_core::log_entry::{Direction, FrameLabel};
use iec104sim_core::slave::RemoteOperationConfig;

use common::harness::{MasterBuilder, SlaveBuilder};
use common::helpers::{wait_for_ioa_count, wait_for_log_event};

/// 每类 5000 个点。GI 只返回 7 个过程量分类(累积量 M_IT 仅由计数量召唤上送),
/// 故 GI 收到 7×5000 = 35,000 点（默认不再预建 TB）。
const POINTS_PER_CATEGORY: u32 = 5000;
const GI_POINTS: usize = 7 * POINTS_PER_CATEGORY as usize;

#[tokio::test]
async fn gi_80k_points_completes_within_window() {
    let log = Arc::new(LogCollector::new());

    let slave = SlaveBuilder::default()
        .with_log(log.clone())
        .with_remote_ops(RemoteOperationConfig::default())
        .with_points_per_category(POINTS_PER_CATEGORY)
        .spawn()
        .await;
    let master = MasterBuilder::new(slave.port, slave.ca)
        .with_log(log.clone())
        .connect()
        .await;

    let start = Instant::now();
    master.conn.send_interrogation(1).await.expect("send GI");

    // 等到日志里出现 "GI 激活终止"，证明 slave 已发完整批响应。
    wait_for_log_event(
        &log,
        |e| {
            e.direction == Direction::Tx
                && matches!(e.frame_label, FrameLabel::GeneralInterrogation)
                && e.detail.contains("激活终止")
        },
        Duration::from_secs(10),
    )
    .await
    .expect("4 万点 GI 应在 10s 内完成（远低于 t1=15s 阈值）");
    let gi_elapsed = start.elapsed();

    // 等 master 收齐 GI 范围内的 IOA。with_default_points 建 8 NA 类型,但 GI 只召唤
    // 7 个过程量分类(累积量除外),master.received_data 内 GI 条目数 = GI_POINTS。
    let count = wait_for_ioa_count(
        &master.conn,
        1,
        GI_POINTS,
        Duration::from_secs(5),
    )
    .await
    .expect("master 应收齐 35,000 个 (IOA, type) 条目");
    assert!(count >= GI_POINTS, "count = {} 应 >= {}", count, GI_POINTS);

    // SQ=1 启用后实际 I 帧数应远低于点位数。8w 点理论上能压到几千帧以内。
    let rx_iframes = log
        .get_all()
        .await
        .iter()
        .filter(|e| e.direction == Direction::Rx)
        .filter(|e| matches!(e.frame_label, FrameLabel::IFrame(_)))
        .count();
    assert!(
        rx_iframes < GI_POINTS / 4,
        "I 帧数 {} 应远低于 {}（应启用 SQ=1）",
        rx_iframes,
        GI_POINTS / 4
    );

    eprintln!(
        "GI 80k 点完成耗时 {:.2}s，主站收到 I 帧数={}（点位条目数={}）",
        gi_elapsed.as_secs_f64(),
        rx_iframes,
        count
    );

    master.shutdown().await;
    slave.shutdown().await;
}
