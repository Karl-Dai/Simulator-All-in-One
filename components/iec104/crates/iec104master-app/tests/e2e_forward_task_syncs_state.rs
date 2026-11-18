//! 端到端复现:fake TCP slave 按金风现场发 `M_DP_NA_1 CA=3 N=0`,
//! 真实的 forward task 逻辑(从 commands.rs 抽出)应当把 CA=3 同步到
//! MockConnState.common_addresses,让 list_connections 暴露给前端。
//!
//! 这是 commands.rs 里 forward task 闭包的纯 Rust 等价版本(不依赖
//! tauri::AppHandle),用来定位"v1.10.3 已恢复学 N=0 但前端仍不显示"
//! 的真实 root cause。

use iec104sim_core::ca_debouncer;
use iec104sim_core::log_collector::LogCollector;
use iec104sim_core::master::{MasterConfig, MasterConnection};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tokio::time::sleep;

// Mock 真实 MockConnState 结构(state 模块 private,测试里 1:1 复制其字段)
struct MockConnState {
    connection: MasterConnection,
    common_addresses: Vec<u16>,
    #[allow(dead_code)]
    log_collector: Arc<LogCollector>,
}

fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

fn iframe(asdu_body: &[u8], ssn: u16, rsn: u16) -> Vec<u8> {
    let len = 4 + asdu_body.len();
    let ssn_le = ssn << 1;
    let rsn_le = rsn << 1;
    let mut f = vec![
        0x68,
        len as u8,
        (ssn_le & 0xFF) as u8,
        (ssn_le >> 8) as u8,
        (rsn_le & 0xFF) as u8,
        (rsn_le >> 8) as u8,
    ];
    f.extend_from_slice(asdu_body);
    f
}

#[tokio::test]
async fn n_zero_dp_frame_propagates_to_state_common_addresses() {
    let port = free_port();

    // Fake slave 按金风 10:23 / 10:46 现场日志精确回 M_DP_NA_1 CA=3 N=0
    let listener = TcpListener::bind(format!("127.0.0.1:{}", port))
        .await
        .unwrap();
    tokio::spawn(async move {
        let (mut sock, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 1024];
        let mut my_ssn: u16 = 0;
        let mut peer_ssn: u16 = 0;
        loop {
            let n = match sock.read(&mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(n) => n,
            };
            let mut i = 0;
            while i < n {
                if buf[i] != 0x68 {
                    i += 1;
                    continue;
                }
                let len = buf[i + 1] as usize;
                let end = i + 2 + len;
                if end > n {
                    break;
                }
                let ctrl1 = buf[i + 2];
                if ctrl1 & 0x03 == 0x03 {
                    if ctrl1 == 0x07 {
                        let _ = sock
                            .write_all(&[0x68, 0x04, 0x0B, 0x00, 0x00, 0x00])
                            .await;
                    }
                } else if ctrl1 & 0x01 == 0 {
                    peer_ssn = peer_ssn.wrapping_add(1);
                    let typeid = buf[i + 6];
                    if typeid == 100 {
                        // 回:ActCon (CA=0xFFFF) + M_DP_NA_1 CA=3 N=0 + ActTerm
                        let bodies: Vec<Vec<u8>> = vec![
                            vec![0x64, 0x01, 0x07, 0x00, 0xFF, 0xFF, 0x00, 0x00, 0x00, 0x14],
                            vec![0x03, 0x80, 0x14, 0x00, 0x03, 0x00],
                            vec![0x64, 0x01, 0x0A, 0x00, 0xFF, 0xFF, 0x00, 0x00, 0x00, 0x14],
                        ];
                        for body in bodies {
                            let f = iframe(&body, my_ssn, peer_ssn);
                            let _ = sock.write_all(&f).await;
                            my_ssn = my_ssn.wrapping_add(1);
                            sleep(Duration::from_millis(10)).await;
                        }
                    }
                }
                i = end;
            }
        }
    });

    sleep(Duration::from_millis(200)).await;

    // 模拟 AppState
    let connections: Arc<RwLock<HashMap<String, MockConnState>>> =
        Arc::new(RwLock::new(HashMap::new()));

    // 模拟 commands.rs::create_connection 流程
    let (ca_inbox, mut flush_rx, _h) = ca_debouncer::spawn(Duration::from_secs(1));
    let log_collector = Arc::new(LogCollector::new());
    let connection = MasterConnection::new(MasterConfig {
        target_address: "127.0.0.1".into(),
        port,
        common_address: 1,
        broadcast_address: 0xFFFF,
        ..Default::default()
    })
    .with_log_collector(log_collector.clone())
    .with_ca_inbox(ca_inbox);
    connection.set_configured_cas(vec![1]);

    let id = "conn_test".to_string();
    connections.write().await.insert(
        id.clone(),
        MockConnState {
            connection,
            log_collector,
            common_addresses: vec![1],
        },
    );

    // **完全照搬 commands.rs forward task 逻辑**(纯 Rust,不依赖 AppHandle)
    let connections_clone = connections.clone();
    let id_clone = id.clone();
    let forward = tokio::spawn(async move {
        let mut emit_log: Vec<(Vec<u16>, Vec<u16>)> = Vec::new();
        while let Some(ev) = flush_rx.recv().await {
            let (added, all_cas) = {
                let mut guard = connections_clone.write().await;
                let Some(c) = guard.get_mut(&id_clone) else { break };
                let added = c.connection.extend_configured_cas(&ev.new_cas);
                if !added.is_empty() {
                    c.common_addresses.extend(added.iter().copied());
                }
                let all_cas = if added.is_empty() {
                    Vec::new()
                } else {
                    c.connection.configured_cas()
                };
                (added, all_cas)
            };
            if !added.is_empty() {
                emit_log.push((added.clone(), all_cas.clone()));
                eprintln!(
                    "[forward-task] would emit connection-cas-updated added={:?} all_cas={:?}",
                    added, all_cas
                );
            }
        }
        emit_log
    });

    // 现在调用 connect + send_interrogation(0xFFFF) — 模拟 connect_master + 广播 GI
    {
        let mut guard = connections.write().await;
        let c = guard.get_mut(&id).unwrap();
        c.connection.connect().await.unwrap();
    }
    sleep(Duration::from_millis(300)).await;

    {
        let guard = connections.read().await;
        let c = guard.get(&id).unwrap();
        c.connection.send_interrogation(0xFFFF).await.unwrap();
    }

    // 给 fake slave 时间发完帧 + debouncer 1s 安静期
    sleep(Duration::from_millis(2500)).await;

    // 看 list_connections 等价物会拿到什么(MockConnState.common_addresses)
    let final_state = {
        let guard = connections.read().await;
        guard.get(&id).map(|c| c.common_addresses.clone())
    };
    eprintln!("================ E2E RESULT ================");
    eprintln!("MockConnState.common_addresses: {:?}", final_state);
    eprintln!("============================================");

    let cas = final_state.expect("connection should still exist");
    assert!(
        cas.contains(&3),
        "BUG: forward task did not propagate CA=3 from debouncer into MockConnState.common_addresses. Final = {:?}",
        cas
    );

    // 等 forward task 结束(把 connection 删了让 flush_rx 自然关)
    connections.write().await.remove(&id);
    let _ = tokio::time::timeout(Duration::from_secs(2), forward).await;
}
