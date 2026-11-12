//! 真实 TCP 端到端测试。
//!
//! 主站与子站通过 `run_master` 与 `run_slave` 在本地回环上互发帧，覆盖链路
//! 复位、总召唤、遥控、突发点位更新与时钟同步。

use std::time::Duration;

use iec101_core::asdu::{AsduAddress, Cp56Time2a};
use iec101_core::link::LinkParams;
use iec101_core::session::{
    run_master, run_slave, MasterEvent101, MasterRequest, MasterStation, PointKind, PointValue,
    SlaveCommand, SlaveStation, StationConfig, StationEvent,
};
use iec101_core::transport::FrameConnection;
use tokio::net::TcpListener;
use tokio::sync::mpsc;

const LINK_ADDRESS: u16 = 1;

fn config() -> StationConfig {
    StationConfig {
        link: LinkParams {
            address: LINK_ADDRESS,
            address_len: 1,
            max_retries: 3,
        },
        address: AsduAddress {
            common_address: 1,
            ca_len: 1,
            ioa_len: 2,
        },
        poll_interval_ms: 10,
        response_timeout_ms: 3000,
    }
}

/// 持续收事件直到满足条件，返回途中收到的全部事件。
async fn collect_until<T, F>(events: &mut mpsc::Receiver<T>, predicate: F, what: &str) -> Vec<T>
where
    F: Fn(&T) -> bool,
{
    let mut seen = Vec::new();
    loop {
        match events.recv().await {
            Some(event) => {
                let matched = predicate(&event);
                seen.push(event);
                if matched {
                    return seen;
                }
            }
            None => panic!("事件通道已关闭，等待 {}", what),
        }
    }
}

#[tokio::test]
async fn tcp_end_to_end_interrogation_control_and_clock_sync() {
    tokio::time::timeout(Duration::from_secs(5), async {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap().to_string();

        // 子站：3 个点位，初始值由 `set_value` 写入。
        let mut slave_station = SlaveStation::new(
            config(),
            vec![
                (1, PointKind::SinglePoint),
                (2, PointKind::DoublePoint),
                (3, PointKind::ShortFloat),
            ],
        );
        assert!(slave_station.set_value(1, PointValue::Bool(true)));
        assert!(slave_station.set_value(2, PointValue::Double(1)));
        assert!(slave_station.set_value(3, PointValue::Float(12.5)));

        let (slave_cmd_tx, slave_cmd_rx) = mpsc::channel::<SlaveCommand>(16);
        let (slave_event_tx, _slave_event_rx) = mpsc::channel::<StationEvent>(64);
        let slave_handle = tokio::spawn(async move {
            let connection = FrameConnection::accept_tcp(&listener).await.unwrap();
            run_slave(connection, slave_station, slave_cmd_rx, slave_event_tx).await
        });

        // 主站。
        let (request_tx, request_rx) = mpsc::channel::<MasterRequest>(16);
        let (master_event_tx, mut master_event_rx) = mpsc::channel::<MasterEvent101>(64);
        let master_handle = tokio::spawn(async move {
            let connection = FrameConnection::connect_tcp(&addr).await.unwrap();
            let station = MasterStation::new(config());
            run_master(connection, station, request_rx, master_event_tx).await
        });

        // 等待链路可用。
        collect_until(
            &mut master_event_rx,
            |e| matches!(e, MasterEvent101::LinkOk),
            "LinkOk",
        )
        .await;

        // 总召唤。
        request_tx
            .send(MasterRequest::Interrogate { qoi: 20 })
            .await
            .unwrap();
        let interrogation = collect_until(
            &mut master_event_rx,
            |e| matches!(e, MasterEvent101::InterrogationFinished),
            "InterrogationFinished",
        )
        .await;
        assert!(interrogation.iter().any(|e| matches!(
            e,
            MasterEvent101::PointUpdate {
                ioa: 1,
                value: PointValue::Bool(true)
            }
        )));
        assert!(interrogation.iter().any(|e| matches!(
            e,
            MasterEvent101::PointUpdate {
                ioa: 2,
                value: PointValue::Double(1)
            }
        )));
        assert!(interrogation.iter().any(|e| matches!(
            e,
            MasterEvent101::PointUpdate {
                ioa: 3,
                value: PointValue::Float(12.5)
            }
        )));

        // 遥控：把点 1 从 true 改成 false。
        request_tx
            .send(MasterRequest::Control { ioa: 1, on: false })
            .await
            .unwrap();
        let control = collect_until(
            &mut master_event_rx,
            |e| matches!(e, MasterEvent101::ControlConfirmed { ioa: 1, on: false }),
            "ControlConfirmed",
        )
        .await;
        assert!(control.iter().any(|e| matches!(
            e,
            MasterEvent101::PointUpdate {
                ioa: 1,
                value: PointValue::Bool(false)
            }
        )));

        // 时钟同步。
        let time = Cp56Time2a {
            milliseconds: 12345,
            minute: 30,
            hour: 12,
            day: 15,
            month: 6,
            year: 2026,
            invalid: false,
            summer_time: false,
        };
        request_tx
            .send(MasterRequest::SyncClock { time })
            .await
            .unwrap();
        collect_until(
            &mut master_event_rx,
            |e| matches!(e, MasterEvent101::ClockSynced),
            "ClockSynced",
        )
        .await;

        // 关闭通道，取回两侧站点做最终断言。
        drop(request_tx);
        drop(slave_cmd_tx);
        let master_station = master_handle.await.unwrap().unwrap();
        let slave_station = slave_handle.await.unwrap().unwrap();

        assert_eq!(slave_station.value(1), Some(PointValue::Bool(false)));
        assert_eq!(slave_station.last_clock_sync(), Some(12345));

        let received = master_station.received();
        assert!(received.contains(&(1, PointValue::Bool(true))));
        assert!(received.contains(&(1, PointValue::Bool(false))));
        assert!(received.contains(&(2, PointValue::Double(1))));
        assert!(received.contains(&(3, PointValue::Float(12.5))));

        let master_stats = master_station.stats();
        let slave_stats = slave_station.stats();
        assert_eq!(master_stats.errors, 0, "主站错误计数应为 0");
        assert_eq!(slave_stats.errors, 0, "子站错误计数应为 0");
        assert!(master_stats.frames_rx > 0, "主站应收到帧");
        assert!(master_stats.frames_tx > 0, "主站应发出帧");
        assert!(slave_stats.frames_rx > 0, "子站应收到帧");
        assert!(slave_stats.frames_tx > 0, "子站应发出帧");
    })
    .await
    .expect("TCP 端到端测试超时");
}
