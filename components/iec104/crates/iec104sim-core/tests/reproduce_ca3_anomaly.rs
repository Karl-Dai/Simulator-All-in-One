//! 受控复现:用裸 TCP fake slave 精确按 Goldwind 现场日志 (2026-05-29 09:54:53)
//! 的 hex 序列回 8 个帧,验证 master 在收到 `CA=0xFFFF` 广播 GI 应答后,
//! `configured_cas` 学到的是 {1, 4} 还是包含意外的 3。
//!
//! 如果输出里 `Final configured_cas` 包含 3,说明代码会从这段精确的字节
//! 序列里"凭空"学出 CA=3,是 bug。如果只学到 4,那么生产环境中 CA=3 来源
//! 是用户场景特有的额外帧(本测试无法重现)。

use iec104sim_core::ca_debouncer;
use iec104sim_core::master::{MasterConfig, MasterConnection};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::time::sleep;

fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

/// 严格按生产日志的 ASDU body 拼一个 I 帧。SSN/RSN 由调用方填。
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
async fn reproduce_production_gi_log_no_ca3() {
    let port = free_port();

    // 起 fake slave,精确按生产日志回 8 个帧
    let listener = TcpListener::bind(format!("127.0.0.1:{}", port))
        .await
        .unwrap();
    let _slave = tokio::spawn(async move {
        let (mut sock, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 1024];
        let mut my_ssn: u16 = 0; // slave 发出的 SSN
        let mut peer_ssn: u16 = 0; // 用来做 RSN(对 master 已发 I 帧的 ack)
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
                let frame_type_u = ctrl1 & 0x03 == 0x03;
                let frame_type_i = ctrl1 & 0x01 == 0;
                if frame_type_u {
                    // STARTDT_ACT = 0x07 → STARTDT_CON = 0x0B
                    // TESTFR_ACT = 0x43 → TESTFR_CON = 0x83
                    if ctrl1 == 0x07 {
                        let _ = sock
                            .write_all(&[0x68, 0x04, 0x0B, 0x00, 0x00, 0x00])
                            .await;
                    } else if ctrl1 == 0x43 {
                        let _ = sock
                            .write_all(&[0x68, 0x04, 0x83, 0x00, 0x00, 0x00])
                            .await;
                    }
                } else if frame_type_i {
                    // master 发的 I 帧: byte 6 = TypeID
                    peer_ssn = peer_ssn.wrapping_add(1);
                    let typeid = buf[i + 6];
                    if typeid == 100 {
                        // GI Act → 按生产日志精确回 8 帧
                        let bodies: Vec<Vec<u8>> = vec![
                            // 1. ActCon (TypeID=100, COT=7, CA=0xFFFF)
                            vec![0x64, 0x01, 0x07, 0x00, 0xFF, 0xFF, 0x00, 0x00, 0x00, 0x14],
                            // 2. SP (TypeID=1, VSQ=0x82 SQ=1 N=2, CA=1, 起始 IOA=1)
                            vec![
                                0x01, 0x82, 0x14, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00,
                            ],
                            // 3. DP 空 ASDU (TypeID=3, VSQ=0x80 SQ=1 N=0, CA=1)
                            vec![0x03, 0x80, 0x14, 0x00, 0x01, 0x00],
                            // 4. ME_NA (TypeID=9, VSQ=0x82, CA=1, 起始 IOA=5)
                            vec![
                                0x09, 0x82, 0x14, 0x00, 0x01, 0x00, 0x05, 0x00, 0x00, 0x05, 0x00,
                                0x00, 0x08, 0x00, 0x00,
                            ],
                            // 5. ME_NB (TypeID=11, VSQ=0x82, CA=1, 起始 IOA=7)
                            vec![
                                0x0B, 0x82, 0x14, 0x00, 0x01, 0x00, 0x07, 0x00, 0x00, 0xF1, 0x00,
                                0x00, 0xD2, 0x01, 0x00,
                            ],
                            // 6. ME_NC CA=1 (TypeID=13, VSQ=0x81, CA=1, IOA=9, float=8F C2 D5 3F)
                            vec![
                                0x0D, 0x81, 0x14, 0x00, 0x01, 0x00, 0x09, 0x00, 0x00, 0x8F, 0xC2,
                                0xD5, 0x3F, 0x00,
                            ],
                            // 7. ME_NC CA=4 (TypeID=13, VSQ=0x81, CA=4, IOA=10, float=E1 FA 9B 42)
                            vec![
                                0x0D, 0x81, 0x14, 0x00, 0x04, 0x00, 0x0A, 0x00, 0x00, 0xE1, 0xFA,
                                0x9B, 0x42, 0x00,
                            ],
                            // 8. ActTerm (TypeID=100, COT=10, CA=0xFFFF)
                            vec![0x64, 0x01, 0x0A, 0x00, 0xFF, 0xFF, 0x00, 0x00, 0x00, 0x14],
                        ];
                        for body in bodies {
                            let f = iframe(&body, my_ssn, peer_ssn);
                            eprintln!(
                                "[fake-slave] tx I frame TypeID={} CA={} body_hex={}",
                                body[0],
                                u16::from_le_bytes([body[4], body[5]]),
                                body.iter()
                                    .map(|b| format!("{:02X}", b))
                                    .collect::<Vec<_>>()
                                    .join(" ")
                            );
                            let _ = sock.write_all(&f).await;
                            my_ssn = my_ssn.wrapping_add(1);
                            sleep(Duration::from_millis(15)).await;
                        }
                    }
                }
                i = end;
            }
        }
    });

    sleep(Duration::from_millis(200)).await;

    // 起 master:configured_cas=[1],broadcast_address=0xFFFF,装 debouncer 1 秒安静期(加速测试)
    let (inbox, mut flush_rx, _h) = ca_debouncer::spawn(Duration::from_secs(1));

    let mut master = MasterConnection::new(MasterConfig {
        target_address: "127.0.0.1".into(),
        port,
        common_address: 1,
        broadcast_address: 0xFFFF,
        ..Default::default()
    })
    .with_ca_inbox(inbox);
    master.set_configured_cas(vec![1]);

    master.connect().await.unwrap();
    sleep(Duration::from_millis(300)).await;

    eprintln!("[master] sending broadcast GI to CA=0xFFFF");
    master.send_interrogation(0xFFFF).await.unwrap();

    // 给 fake slave 时间发完 8 帧 + debouncer 安静期
    sleep(Duration::from_millis(2000)).await;

    let mut flushed: Vec<u16> = Vec::new();
    while let Ok(ev) = flush_rx.try_recv() {
        flushed.extend(ev.new_cas);
    }
    flushed.sort();
    flushed.dedup();

    let final_cas = master.configured_cas();
    eprintln!("================ RESULT ================");
    eprintln!("Debouncer flushed new CAs: {:?}", flushed);
    eprintln!("Final master.configured_cas: {:?}", final_cas);
    eprintln!(
        "Stored data by CA: {:?}",
        master.received_data.read().await.cas()
    );
    eprintln!("========================================");

    let stored_cas = master.received_data.read().await.cas();

    // Debouncer 必须 flush 出 [4](生产 hex 序列中唯一的"未配置 CA")
    assert_eq!(
        flushed,
        vec![4],
        "BUG: debouncer should flush exactly [4] from production hex sequence, got {:?}",
        flushed
    );
    // 数据入桶 CA=1 和 CA=4
    assert!(
        stored_cas.contains(&1) && stored_cas.contains(&4),
        "BUG: expected by_ca to have 1 and 4, got {:?}",
        stored_cas
    );
    // CA=3 在任何路径上都不应出现 —— 生产 hex 序列里没有
    assert!(
        !flushed.contains(&3),
        "BUG: CA=3 leaked into debouncer flush set: {:?}",
        flushed
    );
    assert!(
        !stored_cas.contains(&3),
        "BUG: CA=3 leaked into by_ca store: {:?}",
        stored_cas
    );
}

/// 第二组测试:模拟从站对广播 GI 用"错误的"自己 CA(而不是 echoed 0xFFFF)
/// 回 ActCon/ActTerm —— 这是一些工业从站的常见协议异常。
/// 验证 master 是否会把这种 ActCon 的 CA 也学进 debouncer。
#[tokio::test]
async fn slave_replies_actcon_with_own_ca_master_learns_it() {
    let port = free_port();

    let listener = TcpListener::bind(format!("127.0.0.1:{}", port))
        .await
        .unwrap();
    let _slave = tokio::spawn(async move {
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
                        // ActCon 用 CA=3(协议异常,应该用 echo 0xFFFF) + ActTerm 用 CA=3
                        let bodies: Vec<Vec<u8>> = vec![
                            vec![0x64, 0x01, 0x07, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x14],
                            vec![0x64, 0x01, 0x0A, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x14],
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

    let (inbox, mut flush_rx, _h) = ca_debouncer::spawn(Duration::from_secs(1));
    let mut master = MasterConnection::new(MasterConfig {
        target_address: "127.0.0.1".into(),
        port,
        common_address: 1,
        broadcast_address: 0xFFFF,
        ..Default::default()
    })
    .with_ca_inbox(inbox);
    master.set_configured_cas(vec![1]);
    master.connect().await.unwrap();
    sleep(Duration::from_millis(300)).await;

    master.send_interrogation(0xFFFF).await.unwrap();
    sleep(Duration::from_millis(2000)).await;

    let mut flushed: Vec<u16> = Vec::new();
    while let Ok(ev) = flush_rx.try_recv() {
        flushed.extend(ev.new_cas);
    }
    flushed.sort();
    flushed.dedup();

    let stored = master.received_data.read().await.cas();
    eprintln!(
        "[anomaly-test] flushed={:?}, stored_by_ca={:?}",
        flushed, stored
    );

    // v1.10.2 起 filter_unknown_ca 跳过 TypeID 100/101/103(命令响应),所以
    // 异常 ActCon CA 不再被学。这避免了从站协议异常污染连接树。
    assert!(
        !flushed.contains(&3),
        "v1.10.2+: ActCon (TypeID=100) CA must NOT be learned even if slave fills its own CA. flushed={:?}",
        flushed
    );
    assert!(
        !stored.contains(&3),
        "by_ca[3] should be empty since ActCon has no data points. stored={:?}",
        stored
    );
}

/// 第三组测试:精确重现 Goldwind 现场 (2026-05-29 10:23:47) 通信日志里的
/// `M_DP_NA_1 CA=3 N=0` 协议违反帧 —— 声称是 CA=3 的双点总召响应,但 N=0,
/// 实际无任何 information object。
/// v1.10.3 起,N=0 数据帧的 CA 仍要学(让用户能在树里看到这个 CA,空节点
/// 本身就是从站协议异常的可视化信号)。
#[tokio::test]
async fn n_zero_dp_frame_ca_is_still_learned() {
    let port = free_port();
    let listener = TcpListener::bind(format!("127.0.0.1:{}", port))
        .await
        .unwrap();
    let _slave = tokio::spawn(async move {
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
                        // 1. ActCon CA=0xFFFF (合规)
                        // 2. M_DP_NA_1 CA=3 N=0 (Goldwind 真实异常帧:声称数据但 N=0)
                        // 3. ActTerm CA=0xFFFF
                        let bodies: Vec<Vec<u8>> = vec![
                            vec![0x64, 0x01, 0x07, 0x00, 0xFF, 0xFF, 0x00, 0x00, 0x00, 0x14],
                            // 完全照搬 Goldwind 日志的 hex: 03 80 14 00 03 00
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

    let (inbox, mut flush_rx, _h) = ca_debouncer::spawn(Duration::from_secs(1));
    let mut master = MasterConnection::new(MasterConfig {
        target_address: "127.0.0.1".into(),
        port,
        common_address: 1,
        broadcast_address: 0xFFFF,
        ..Default::default()
    })
    .with_ca_inbox(inbox);
    master.set_configured_cas(vec![1]);
    master.connect().await.unwrap();
    sleep(Duration::from_millis(300)).await;

    master.send_interrogation(0xFFFF).await.unwrap();
    sleep(Duration::from_millis(2000)).await;

    let mut flushed: Vec<u16> = Vec::new();
    while let Ok(ev) = flush_rx.try_recv() {
        flushed.extend(ev.new_cas);
    }
    flushed.sort();
    flushed.dedup();

    eprintln!("[n=0 test] flushed={:?}", flushed);
    // v1.10.3 起:N=0 数据帧的 CA 仍要学,让用户能在树里看到这个 CA(即使没数据)。
    // 空节点本身就是从站协议异常的可视化信号。
    assert_eq!(
        flushed,
        vec![3],
        "v1.10.3+: N=0 data frame CA must still be learned so the user can see the station node in the tree. flushed={:?}",
        flushed
    );
}
