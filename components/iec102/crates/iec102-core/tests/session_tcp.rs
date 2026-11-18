//! 真实 TCP 端到端测试。
//!
//! 主站与子站通过 `run_master102` 与 `run_slave102` 在本地回环上互发帧，覆盖
//! 链路复位、累计量召唤（冻结方式 0/1/2）、读数修改与顺序号变化。

use std::time::Duration;

use iec102_core::asdu::{Qcc, StationAddress};
use iec102_core::link::LinkParams;
use iec102_core::session::{
    run_master102, run_slave102, CounterPoint, MasterEvent102, MasterRequest102, MasterStation102,
    SlaveStation102, StationConfig102, StationEvent102,
};
use iec102_core::transport::FrameConnection;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

const LINK_ADDRESS: u16 = 1;

type MasterHandle = JoinHandle<std::io::Result<MasterStation102>>;
type SlaveHandle = JoinHandle<std::io::Result<SlaveStation102>>;

fn config() -> StationConfig102 {
    StationConfig102 {
        link: LinkParams {
            address: LINK_ADDRESS,
            address_len: 1,
            max_retries: 3,
        },
        address: StationAddress {
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

/// 启动一对主站与子站，返回请求发送端、两侧事件接收端与两个任务句柄。
async fn start_session(
    slave_station: SlaveStation102,
) -> (
    mpsc::Sender<MasterRequest102>,
    mpsc::Receiver<MasterEvent102>,
    mpsc::Receiver<StationEvent102>,
    MasterHandle,
    SlaveHandle,
) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap().to_string();

    let (slave_event_tx, slave_event_rx) = mpsc::channel::<StationEvent102>(64);
    let slave_handle = tokio::spawn(async move {
        let connection = FrameConnection::accept_tcp(&listener).await.unwrap();
        run_slave102(connection, slave_station, slave_event_tx).await
    });

    let (request_tx, request_rx) = mpsc::channel::<MasterRequest102>(16);
    let (master_event_tx, master_event_rx) = mpsc::channel::<MasterEvent102>(64);
    let master_handle = tokio::spawn(async move {
        let connection = FrameConnection::connect_tcp(&addr).await.unwrap();
        let station = MasterStation102::new(config());
        run_master102(connection, station, request_rx, master_event_tx).await
    });

    (
        request_tx,
        master_event_rx,
        slave_event_rx,
        master_handle,
        slave_handle,
    )
}

/// 关闭请求通道，取回两侧站点。
async fn finish_session(
    request_tx: mpsc::Sender<MasterRequest102>,
    master_handle: MasterHandle,
    slave_handle: SlaveHandle,
) -> (MasterStation102, SlaveStation102) {
    drop(request_tx);
    let master = master_handle.await.unwrap().unwrap();
    let slave = slave_handle.await.unwrap().unwrap();
    (master, slave)
}

#[tokio::test]
async fn tcp_end_to_end_counter_interrogation() {
    tokio::time::timeout(Duration::from_secs(5), async {
        // 第一阶段：冻结方式 0 与 1。
        let points = vec![
            CounterPoint {
                ioa: 1,
                reading: 100,
            },
            CounterPoint {
                ioa: 2,
                reading: 200,
            },
            CounterPoint {
                ioa: 3,
                reading: 300,
            },
        ];
        let slave_station = SlaveStation102::new(config(), points);
        let (request_tx, mut master_rx, mut slave_rx, master_handle, slave_handle) =
            start_session(slave_station).await;

        collect_until(
            &mut master_rx,
            |e| matches!(e, MasterEvent102::LinkOk),
            "LinkOk",
        )
        .await;

        // 冻结方式 0：直接读当前值。
        request_tx
            .send(MasterRequest102::Interrogate {
                qcc: Qcc {
                    request: 0,
                    freeze: 0,
                },
            })
            .await
            .unwrap();
        let first = collect_until(
            &mut master_rx,
            |e| matches!(e, MasterEvent102::InterrogationFinished),
            "第一次召唤结束",
        )
        .await;
        assert!(first.iter().any(|e| matches!(
            e,
            MasterEvent102::CounterUpdate {
                ioa: 1,
                reading: 100,
                sequence: 0
            }
        )));
        assert!(first.iter().any(|e| matches!(
            e,
            MasterEvent102::CounterUpdate {
                ioa: 2,
                reading: 200,
                sequence: 0
            }
        )));
        assert!(first.iter().any(|e| matches!(
            e,
            MasterEvent102::CounterUpdate {
                ioa: 3,
                reading: 300,
                sequence: 0
            }
        )));

        // 冻结方式 1：冻结并读，读数不变。
        request_tx
            .send(MasterRequest102::Interrogate {
                qcc: Qcc {
                    request: 0,
                    freeze: 1,
                },
            })
            .await
            .unwrap();
        let second = collect_until(
            &mut master_rx,
            |e| matches!(e, MasterEvent102::InterrogationFinished),
            "第二次召唤结束",
        )
        .await;
        assert!(second.iter().any(|e| matches!(
            e,
            MasterEvent102::CounterUpdate {
                ioa: 1,
                reading: 100,
                sequence: 0
            }
        )));
        let frozen = collect_until(
            &mut slave_rx,
            |e| matches!(e, StationEvent102::CounterFrozen { ioa: 1, .. }),
            "子站 CounterFrozen",
        )
        .await;
        assert!(frozen.iter().any(|e| matches!(
            e,
            StationEvent102::CounterFrozen {
                ioa: 1,
                reading: 100
            }
        )));

        let (master_station, mut slave_station) =
            finish_session(request_tx, master_handle, slave_handle).await;
        let received = master_station.received();
        assert!(received.contains(&(1, 100, 0)), "主站应收到点 1");
        assert!(received.contains(&(2, 200, 0)), "主站应收到点 2");
        assert!(received.contains(&(3, 300, 0)), "主站应收到点 3");

        // 第二阶段：修改读数后再召唤。
        assert!(slave_station.set_reading(1, 150), "点 1 应存在");
        let (request_tx, mut master_rx, mut slave_rx, master_handle, slave_handle) =
            start_session(slave_station).await;
        collect_until(
            &mut master_rx,
            |e| matches!(e, MasterEvent102::LinkOk),
            "LinkOk",
        )
        .await;

        request_tx
            .send(MasterRequest102::Interrogate {
                qcc: Qcc {
                    request: 0,
                    freeze: 0,
                },
            })
            .await
            .unwrap();
        let third = collect_until(
            &mut master_rx,
            |e| matches!(e, MasterEvent102::InterrogationFinished),
            "第三次召唤结束",
        )
        .await;
        assert!(third.iter().any(|e| matches!(
            e,
            MasterEvent102::CounterUpdate {
                ioa: 1,
                reading: 150,
                sequence: 1
            }
        )));

        // 冻结方式 2：冻结并复位读，先读后清零。
        request_tx
            .send(MasterRequest102::Interrogate {
                qcc: Qcc {
                    request: 0,
                    freeze: 2,
                },
            })
            .await
            .unwrap();
        collect_until(
            &mut master_rx,
            |e| matches!(e, MasterEvent102::InterrogationFinished),
            "第四次召唤结束",
        )
        .await;
        let reset_events = collect_until(
            &mut slave_rx,
            |e| matches!(e, StationEvent102::CounterReset { ioa: 1 }),
            "子站 CounterReset",
        )
        .await;
        assert!(reset_events
            .iter()
            .any(|e| matches!(e, StationEvent102::CounterReset { ioa: 1 })));

        let (master_station, slave_station) =
            finish_session(request_tx, master_handle, slave_handle).await;
        assert!(
            master_station.received().contains(&(1, 150, 1)),
            "主站应收到修改后的读数"
        );
        assert_eq!(slave_station.reading(1), Some(0), "冻结并复位后读数应为 0");
        assert_eq!(slave_station.sequence(1), Some(1), "复位不应改变顺序号");

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
