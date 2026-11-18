//! 验证带时标 ASDU (NA→TB) 在主子站间的字节布局与解码。
//! 重点:slave 自发 TB 帧时含 7 字节 CP56Time2a,master 端解码后 `timestamp` 字段非 None。

mod common;
use common::harness::Pair;
use common::helpers::{collect_iframe_bytes, wait_for_ioa_count, DEFAULT_TIMEOUT};

use iec104sim_core::data_point::DataPointValue;
use iec104sim_core::log_entry::Direction;
use iec104sim_core::slave::RemoteOperationConfig;
use iec104sim_core::types::AsduTypeId;
use tokio::time::{sleep, Duration};

/// `gi_include_timestamped=true` 时 GI 应答中既有 NA 类型(type=1)又有 TB 类型(type=30)。
/// 抽样校验 TB 帧的字节布局:I-frame APCI 6 字节 + ASDU 头 6 字节 + IOA 3 字节 + SIQ 1 字节
/// + CP56Time2a 7 字节 = 23 字节,且 type id 字节为 30。
#[tokio::test]
async fn tb_frames_carry_cp56time2a() {
    let ops = RemoteOperationConfig { gi_include_timestamped: true, ..Default::default() };
    let pair = Pair::spawn_with(ops, 3).await;

    pair.master.conn.send_interrogation(1).await.unwrap();
    // GI 完成:至少要看到 NA + TB 各一份点。
    let _ = wait_for_ioa_count(&pair.master.conn, 1, 2, DEFAULT_TIMEOUT).await;
    sleep(Duration::from_millis(300)).await;

    // 在 master 侧 Rx I-frame 中找 type=30 (M_SP_TB_1) 的帧。
    let frames = collect_iframe_bytes(&pair.log, Direction::Rx).await;
    let tb_sp = frames.iter().find(|f| f.len() >= 7 && f[6] == 30).cloned();
    let frame = tb_sp.expect("应在 master Rx 中看到 M_SP_TB_1 帧");

    // ASDU body 字节布局:
    //   offset 6: type=30
    //   offset 7: VSQ
    //   offset 8: COT (GI 响应 = 20)
    //   offset 9: OA = 0
    //   offset 10..12: CA
    //   offset 12..15: IOA (3 字节)
    //   offset 15: SIQ
    //   offset 16..23: CP56Time2a
    assert_eq!(frame[6], 30, "type 应为 30 (M_SP_TB_1)");
    assert_eq!(frame[8], 20, "GI 响应 COT 应为 20 (Interrogated)");
    // SIQ + CP56 共 8 字节;整帧应 ≥ 23 字节 (含 start + length)
    assert!(frame.len() >= 23, "frame too short: {}", frame.len());

    // master 端 received_data 中,IOA=1 / M_SP_TB_1 这点的 timestamp 应非 None。
    let data = pair.master.conn.received_data.read().await;
    let map = data.ca_map(1).expect("CA=1");
    let tb_point = map.get(1, AsduTypeId::MSpTb1).expect("master 应收到 TB 类型点");
    assert!(tb_point.timestamp.is_some(), "TB 点必须带 timestamp");
    drop(data);

    pair.shutdown().await;
}

/// NA 帧(无时标)的字节长度比 TB 短 7。比较同一 GI 中的 type=1 帧与 type=30 帧长度差。
#[tokio::test]
async fn na_frame_is_7_bytes_shorter_than_tb() {
    let ops = RemoteOperationConfig { gi_include_timestamped: true, ..Default::default() };
    let pair = Pair::spawn_with(ops, 1).await;

    pair.master.conn.send_interrogation(1).await.unwrap();
    let _ = wait_for_ioa_count(&pair.master.conn, 1, 2, DEFAULT_TIMEOUT).await;
    sleep(Duration::from_millis(300)).await;

    let frames = collect_iframe_bytes(&pair.log, Direction::Rx).await;
    let na = frames.iter().find(|f| f.len() >= 7 && f[6] == 1).cloned()
        .expect("找不到 M_SP_NA_1 帧");
    let tb = frames.iter().find(|f| f.len() >= 7 && f[6] == 30).cloned()
        .expect("找不到 M_SP_TB_1 帧");
    assert_eq!(tb.len(), na.len() + 7, "TB 比 NA 多 7 字节时标 (NA={}, TB={})", na.len(), tb.len());

    pair.shutdown().await;
}

/// 默认 ops(同步开关全关) 下,(IOA=1, MSpNa1) 的变位上送只生成 type=1 帧,
/// 不应额外追加 type=30 TB 帧。用 COT=3 过滤 spontaneous 帧,排除 GI 应答噪声。
#[tokio::test]
async fn default_spontaneous_emits_only_na() {
    let pair = Pair::spawn(RemoteOperationConfig::default()).await;
    pair.master.conn.send_interrogation(1).await.unwrap();
    let _ = wait_for_ioa_count(&pair.master.conn, 1, 1, DEFAULT_TIMEOUT).await;

    // 等 GI 完全结束 (seq 在 400 ms 内不增长视为稳定),再清日志。
    let mut last = pair.master.conn.received_data.read().await.current_seq();
    loop {
        sleep(Duration::from_millis(400)).await;
        let cur = pair.master.conn.received_data.read().await.current_seq();
        if cur == last { break; }
        last = cur;
    }
    pair.log.clear().await;

    {
        let mut stations = pair.slave.server.stations.write().await;
        let st = stations.get_mut(&1).unwrap();
        st.data_points
            .get_mut(1, AsduTypeId::MSpNa1)
            .unwrap()
            .value = DataPointValue::SinglePoint { value: true };
        st.data_points.mark_changed(1, AsduTypeId::MSpNa1);
    }
    pair.slave.server
        .queue_spontaneous(1, &[(1, AsduTypeId::MSpNa1)])
        .await;
    sleep(Duration::from_millis(400)).await;

    // 只看 COT=3 (spontaneous) 的帧。
    let frames = collect_iframe_bytes(&pair.log, Direction::Rx).await;
    let spont: Vec<_> = frames.iter().filter(|f| f.len() > 8 && f[8] == 3).collect();
    let has_tb = spont.iter().any(|f| f[6] == 30);
    let has_na = spont.iter().any(|f| f[6] == 1);
    assert!(has_na, "默认 spontaneous 应有 NA type=1, frames={:?}", spont.len());
    assert!(!has_tb, "默认 spontaneous 不应有 TB type=30 (同步开关全关)");

    pair.shutdown().await;
}
