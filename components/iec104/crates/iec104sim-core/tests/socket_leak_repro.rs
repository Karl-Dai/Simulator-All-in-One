//! Repro: a master that connects then disconnects (without STARTDT, queue empty)
//! leaves the slave's socket stuck in CLOSE_WAIT — a file-descriptor leak. Once
//! enough leak, the slave hits its RLIMIT_NOFILE and accept() fails with EMFILE,
//! so new masters can no longer connect ("only one master gets on").

use iec104sim_core::slave::{SlaveServer, SlaveTransportConfig, Station};
use std::net::TcpStream;
use std::time::Duration;
use tokio::time::sleep;

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

/// Count this process's sockets stuck in CLOSE_WAIT on the given local port.
/// Returns None if `lsof` is unavailable so the test can skip on such hosts.
fn close_wait_count(port: u16) -> Option<usize> {
    let pid = std::process::id().to_string();
    let out = std::process::Command::new("lsof")
        .args(["-nP", "-p", &pid])
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&out.stdout);
    let needle = format!(":{}->", port);
    Some(s.lines().filter(|l| l.contains("CLOSE_WAIT") && l.contains(&needle)).count())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn disconnected_master_does_not_leak_sockets() {
    let port = free_port();
    let mut slave = SlaveServer::new(SlaveTransportConfig {
        bind_address: "127.0.0.1".to_string(),
        port,
        ..Default::default()
    });
    slave.add_station(Station::with_default_points(1, "T", 4)).await.unwrap();
    slave.start().await.unwrap();
    sleep(Duration::from_millis(150)).await;

    // 20 masters connect then immediately disconnect, never sending STARTDT
    // (so the per-connection write queue stays empty — worst case for the leak).
    for _ in 0..20 {
        let c = TcpStream::connect(("127.0.0.1", port)).expect("connect");
        // No STARTDT, no data — just close, sending FIN to the slave.
        drop(c);
        sleep(Duration::from_millis(30)).await;
    }

    // Give the slave ample time to notice EOF and tear the sockets down.
    sleep(Duration::from_millis(1500)).await;

    let leaked = match close_wait_count(port) {
        Some(n) => n,
        None => {
            eprintln!("lsof unavailable on this host — skipping CLOSE_WAIT assertion");
            slave.stop().await.unwrap();
            return;
        }
    };
    println!("CLOSE_WAIT sockets still held by slave: {leaked}");
    assert_eq!(leaked, 0, "slave leaked {leaked} CLOSE_WAIT sockets after masters disconnected");

    slave.stop().await.unwrap();
}
