//! 事件驱动等待与断言工具。所有 wait_* 函数返回 `Result<T, String>`,
//! 错误信息包含超时上下文,失败时 panic message 易诊断。

#![allow(dead_code)] // helpers 函数被多个测试文件按需调用

use std::sync::Arc;
use std::time::Duration;

use iec104sim_core::data_point::{DataPointValue};
use iec104sim_core::log_collector::LogCollector;
use iec104sim_core::log_entry::{Direction, FrameLabel, LogEntry};
use iec104sim_core::master::{MasterConnection, MasterState};
use iec104sim_core::slave::SlaveServer;
use iec104sim_core::types::AsduTypeId;

pub const POLL_INTERVAL: Duration = Duration::from_millis(50);
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);
pub const SHORT_TIMEOUT: Duration = Duration::from_millis(800);

/// 等待 master 状态进入 `Connected`。返回 Err 时不影响测试继续(但 harness 会 expect)。
pub async fn wait_for_master_connected(
    master: &MasterConnection,
    timeout: Duration,
) -> Result<(), String> {
    let mut state_rx = master.subscribe_state();
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if matches!(*state_rx.borrow(), MasterState::Connected) {
            return Ok(());
        }
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return Err(format!("master not connected, state={:?}", *state_rx.borrow()));
        }
        let _ = tokio::time::timeout(remaining.min(Duration::from_millis(200)), state_rx.changed()).await;
    }
}

/// 等到 master.received_data 的 (ca) 子表至少出现 `min_count` 个点。
pub async fn wait_for_ioa_count(
    master: &MasterConnection,
    ca: u16,
    min_count: usize,
    timeout: Duration,
) -> Result<usize, String> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let count = {
            let data = master.received_data.read().await;
            data.ca_map(ca).map(|m| m.len()).unwrap_or(0)
        };
        if count >= min_count {
            return Ok(count);
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(format!(
                "timeout waiting for IOAs on CA={}, want >= {}, got {}",
                ca, min_count, count
            ));
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

/// 等待 received_data.seq_counter 前进至少 `delta` 步。
pub async fn wait_for_seq_advance(
    master: &MasterConnection,
    since: u64,
    delta: u64,
    timeout: Duration,
) -> Result<u64, String> {
    let target = since + delta;
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let cur = master.received_data.read().await.current_seq();
        if cur >= target {
            return Ok(cur);
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(format!(
                "timeout waiting for seq advance: since={}, want delta={}, current={}",
                since, delta, cur
            ));
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

/// 在日志中扫匹配的 entry。命中即返回 Ok(entry 副本)。
pub async fn wait_for_log_event(
    log: &Arc<LogCollector>,
    predicate: impl Fn(&LogEntry) -> bool,
    timeout: Duration,
) -> Result<LogEntry, String> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let entries = log.get_all().await;
        if let Some(e) = entries.iter().find(|e| predicate(e)).cloned() {
            return Ok(e);
        }
        if tokio::time::Instant::now() >= deadline {
            return Err("timeout waiting for log event".to_string());
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

/// 统计指定方向 + ASDU type id (字符串,如 "M_SP_NA_1") 的 I-frame 数量。
pub async fn count_iframes(log: &Arc<LogCollector>, dir: Direction, asdu_name: &str) -> usize {
    log.get_all()
        .await
        .iter()
        .filter(|e| e.direction == dir)
        .filter(|e| match &e.frame_label {
            FrameLabel::IFrame(s) => s.contains(asdu_name),
            _ => false,
        })
        .count()
}

/// 收集所有 I-frame 的 raw_bytes(按发送顺序);用于做字节级检查。
pub async fn collect_iframe_bytes(
    log: &Arc<LogCollector>,
    dir: Direction,
) -> Vec<Vec<u8>> {
    log.get_all()
        .await
        .iter()
        .filter(|e| e.direction == dir)
        .filter(|e| matches!(e.frame_label, FrameLabel::IFrame(_)))
        .filter_map(|e| e.raw_bytes.clone())
        .collect()
}

/// 在 slave 内查看某点当前值,失败时返回错误描述。
pub async fn slave_point_value(
    slave: &SlaveServer,
    ca: u16,
    ioa: u32,
    asdu_type: AsduTypeId,
) -> Result<DataPointValue, String> {
    let stations = slave.stations.read().await;
    let st = stations
        .get(&ca)
        .ok_or_else(|| format!("CA={} not found on slave", ca))?;
    let p = st
        .data_points
        .get(ioa, asdu_type)
        .ok_or_else(|| format!("IOA={} type={:?} not found", ioa, asdu_type))?;
    Ok(p.value.clone())
}

/// 在 master.received_data 中查某点的值。
pub async fn master_point_value(
    master: &MasterConnection,
    ca: u16,
    ioa: u32,
    asdu_type: AsduTypeId,
) -> Result<DataPointValue, String> {
    let data = master.received_data.read().await;
    let map = data
        .ca_map(ca)
        .ok_or_else(|| format!("CA={} not seen on master", ca))?;
    let p = map
        .get(ioa, asdu_type)
        .ok_or_else(|| format!("IOA={} type={:?} not seen on master", ioa, asdu_type))?;
    Ok(p.value.clone())
}
