//! Regression test for the STARTDT gating bug.
//!
//! IEC 60870-5-104 requires the controlled station (slave) to send I-frames
//! only while data transfer is active — i.e. after the master issues STARTDT
//! and before STOPDT. The simulator used to register a connection and start
//! firing cyclic / spontaneous I-frames the moment TCP was accepted, before
//! STARTDT. A master in the STOPPED state discards those frames without
//! advancing its receive sequence counter, so the slave's N(S) ends up
//! permanently ahead and the master logs "Receive unexpted I-Frame ns".

use iec104sim_core::slave::{CyclicConfig, SlaveServer, SlaveTransportConfig, Station};
use iec104sim_core::types::AsduTypeId;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;
use tokio::time::sleep;

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// Drain everything currently readable and count I-frames — APDUs whose APCI
/// control byte 0 has bit 0 == 0 (U- and S-frames have bit 0 == 1).
fn count_i_frames(stream: &mut TcpStream) -> usize {
    let mut acc = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => acc.extend_from_slice(&buf[..n]),
            Err(_) => break, // read timeout — nothing more pending
        }
    }
    let mut count = 0;
    let mut i = 0;
    while i + 1 < acc.len() {
        if acc[i] != 0x68 {
            i += 1;
            continue;
        }
        let len = acc[i + 1] as usize + 2;
        if i + len > acc.len() {
            break;
        }
        if len >= 6 && acc[i + 2] & 0x01 == 0 {
            count += 1;
        }
        i += len;
    }
    count
}

#[tokio::test]
async fn slave_withholds_iframes_until_startdt() {
    let port = free_port();

    let mut slave = SlaveServer::new(SlaveTransportConfig {
        bind_address: "127.0.0.1".to_string(),
        port,
        ..Default::default()
    });
    slave
        .add_station(Station::with_default_points(1, "T", 10))
        .await
        .unwrap();
    // Fast cyclic transmission so several ticks elapse during the test.
    slave
        .set_cyclic_config(1, CyclicConfig { enabled: true, interval_ms: 100 })
        .await
        .unwrap();
    slave.start().await.unwrap();
    sleep(Duration::from_millis(200)).await;

    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_millis(150)))
        .unwrap();

    // --- Before STARTDT: the slave must stay silent. ---
    // Let cyclic ticks fire and push a spontaneous burst through queue_spontaneous.
    for _ in 0..5 {
        slave
            .queue_spontaneous(1, &[(1, AsduTypeId::MSpNa1), (2, AsduTypeId::MSpNa1)])
            .await;
        sleep(Duration::from_millis(120)).await;
    }
    let before = count_i_frames(&mut stream);
    assert_eq!(
        before, 0,
        "slave sent {} I-frame(s) before STARTDT — this desyncs the master's N(S)",
        before
    );

    // --- STARTDT: enable data transfer. ---
    stream
        .write_all(&[0x68, 0x04, 0x07, 0x00, 0x00, 0x00])
        .unwrap();
    sleep(Duration::from_millis(250)).await;
    // Drain STARTDT_CON and the first cyclic burst.
    let _ = count_i_frames(&mut stream);

    // --- After STARTDT: I-frames must flow. ---
    for _ in 0..5 {
        slave
            .queue_spontaneous(1, &[(3, AsduTypeId::MSpNa1)])
            .await;
        sleep(Duration::from_millis(120)).await;
    }
    let after = count_i_frames(&mut stream);
    assert!(
        after > 0,
        "slave sent no I-frames after STARTDT — data transfer is broken"
    );

    slave.stop().await.unwrap();
}
