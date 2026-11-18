//! 集成测试:模拟从站用未配置 CA 应答,debouncer 3s 后 flush。

use iec104sim_core::ca_debouncer;
use std::time::Duration;

#[tokio::test(start_paused = true)]
async fn debouncer_collects_three_cas_and_flushes_once() {
    let (inbox, mut rx, _h) = ca_debouncer::spawn(Duration::from_secs(3));
    inbox.push(1);
    inbox.push(2);
    inbox.push(3);
    inbox.push(2); // 重复
    tokio::time::sleep(Duration::from_millis(3100)).await;
    let ev = rx.recv().await.unwrap();
    assert_eq!(ev.new_cas, vec![1, 2, 3]);
    // 没有第二次 flush
    assert!(rx.try_recv().is_err());
}

#[tokio::test(start_paused = true)]
async fn debouncer_handles_burst_then_settle() {
    let (inbox, mut rx, _h) = ca_debouncer::spawn(Duration::from_secs(2));
    for ca in [10u16, 11, 12, 13, 14, 15] {
        inbox.push(ca);
    }
    tokio::time::sleep(Duration::from_millis(2100)).await;
    let ev = rx.recv().await.unwrap();
    assert_eq!(ev.new_cas, (10..=15).collect::<Vec<u16>>());
}
