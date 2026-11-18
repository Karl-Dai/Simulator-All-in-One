//! 验证 `auto_packing=true` 时连续 IOA 同类型点合并成单 ASDU (VSQ.SQ=1)。

mod common;
use common::harness::Pair;
use common::helpers::{collect_iframe_bytes, wait_for_ioa_count, DEFAULT_TIMEOUT};

use iec104sim_core::data_point::DataPointValue;
use iec104sim_core::log_entry::Direction;
use iec104sim_core::slave::{RemoteOperationConfig, UploadMode};
use iec104sim_core::types::AsduTypeId;
use tokio::time::{sleep, Duration};

/// 5 个连续 IOA 同类型 SP 点变位 + auto_packing + Continuous → 单 ASDU 帧 VSQ=0x85。
#[tokio::test]
async fn five_consecutive_sp_packed_into_single_asdu() {
    let ops = RemoteOperationConfig {
        auto_packing: true,
        upload_mode_untimestamped: UploadMode::Continuous,
        ..Default::default()
    };
    let pair = Pair::spawn_with(ops, 10).await;

    pair.master.conn.send_interrogation(1).await.unwrap();
    let _ = wait_for_ioa_count(&pair.master.conn, 1, 1, DEFAULT_TIMEOUT).await;
    pair.log.clear().await;

    // 改 IOA=1..5 的 SP 值,然后 queue_spontaneous 整批。
    let mut changed = Vec::new();
    {
        let mut stations = pair.slave.server.stations.write().await;
        let st = stations.get_mut(&1).unwrap();
        for ioa in 1..=5u32 {
            let p = st.data_points.get_mut(ioa, AsduTypeId::MSpNa1).unwrap();
            p.value = DataPointValue::SinglePoint { value: ioa % 2 == 0 };
            st.data_points.mark_changed(ioa, AsduTypeId::MSpNa1);
            changed.push((ioa, AsduTypeId::MSpNa1));
        }
    }
    pair.slave.server.queue_spontaneous(1, &changed).await;
    sleep(Duration::from_millis(400)).await;

    // 在 master Rx 中找 type=1 且 VSQ.SQ=1 的帧。
    let frames = collect_iframe_bytes(&pair.log, Direction::Rx).await;
    let packed = frames
        .iter()
        .find(|f| f.len() >= 16 && f[6] == 1 && (f[7] & 0x80) != 0)
        .cloned();
    let frame = packed.expect("应有 VSQ.SQ=1 的 M_SP_NA_1 打包帧");
    assert_eq!(frame[7], 0x85, "VSQ 应为 0x85 (SQ=1 + n=5),实际 {:#04x}", frame[7]);
    // IOA 仅写一份(IOA1=1,即 [01,00,00]),后面跟 5 个 SIQ。
    assert_eq!(&frame[12..15], &[1, 0, 0], "IOA 应只写第一个 (1)");
    // 5 个 SIQ:value=ioa%2==0
    let expected_siqs = [0, 1, 0, 1, 0]; // ioa=1..5 → false,true,false,true,false
    for (i, exp) in expected_siqs.iter().enumerate() {
        assert_eq!(frame[15 + i], *exp, "第 {} 个 SIQ", i);
    }

    pair.shutdown().await;
}

/// IOA 不连续时,auto_packing 应回退到逐点路径,不出现 VSQ.SQ=1 的帧。
#[tokio::test]
async fn non_consecutive_ioas_fallback_to_per_point() {
    let ops = RemoteOperationConfig {
        auto_packing: true,
        upload_mode_untimestamped: UploadMode::Continuous,
        ..Default::default()
    };
    let pair = Pair::spawn_with(ops, 10).await;

    pair.master.conn.send_interrogation(1).await.unwrap();
    let _ = wait_for_ioa_count(&pair.master.conn, 1, 1, DEFAULT_TIMEOUT).await;
    pair.log.clear().await;

    // 改 IOA=1,3,5 (跳号) 的 SP 值。
    let ioas = [1u32, 3, 5];
    let mut changed = Vec::new();
    {
        let mut stations = pair.slave.server.stations.write().await;
        let st = stations.get_mut(&1).unwrap();
        for &ioa in &ioas {
            let p = st.data_points.get_mut(ioa, AsduTypeId::MSpNa1).unwrap();
            p.value = DataPointValue::SinglePoint { value: true };
            st.data_points.mark_changed(ioa, AsduTypeId::MSpNa1);
            changed.push((ioa, AsduTypeId::MSpNa1));
        }
    }
    pair.slave.server.queue_spontaneous(1, &changed).await;
    sleep(Duration::from_millis(400)).await;

    // 期望:3 个独立的 type=1 帧 (n=1)。无论 SQ 位如何,只要每帧 n=1 即代表"逐点"。
    let frames = collect_iframe_bytes(&pair.log, Direction::Rx).await;
    let sp_frames: Vec<_> = frames.iter().filter(|f| f.len() >= 16 && f[6] == 1).collect();
    assert!(sp_frames.len() >= 3, "至少 3 个 SP 帧,实际 {}", sp_frames.len());
    // 不应出现 n>1 的合并帧:VSQ 低 7 位即对象数。
    for (i, f) in sp_frames.iter().enumerate() {
        let n = f[7] & 0x7F;
        assert_eq!(n, 1, "帧 {} 应只含 1 个对象,实际 n={}", i, n);
    }

    pair.shutdown().await;
}

/// `auto_packing=false` 默认行为:即使 IOA 连续,也走逐点路径。
#[tokio::test]
async fn auto_packing_disabled_uses_per_point() {
    let pair = Pair::spawn_with(RemoteOperationConfig::default(), 10).await;

    pair.master.conn.send_interrogation(1).await.unwrap();
    let _ = wait_for_ioa_count(&pair.master.conn, 1, 1, DEFAULT_TIMEOUT).await;
    pair.log.clear().await;

    let mut changed = Vec::new();
    {
        let mut stations = pair.slave.server.stations.write().await;
        let st = stations.get_mut(&1).unwrap();
        for ioa in 1..=3u32 {
            let p = st.data_points.get_mut(ioa, AsduTypeId::MSpNa1).unwrap();
            p.value = DataPointValue::SinglePoint { value: true };
            st.data_points.mark_changed(ioa, AsduTypeId::MSpNa1);
            changed.push((ioa, AsduTypeId::MSpNa1));
        }
    }
    pair.slave.server.queue_spontaneous(1, &changed).await;
    sleep(Duration::from_millis(400)).await;

    let frames = collect_iframe_bytes(&pair.log, Direction::Rx).await;
    let packed = frames.iter().any(|f| f.len() >= 8 && f[6] == 1 && (f[7] & 0x80) != 0);
    assert!(!packed, "auto_packing=false 不应出现打包帧");

    pair.shutdown().await;
}
