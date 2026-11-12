//! TCP 与串口传输层集成测试。

use std::time::Duration;

use iec102_core::ft12::{encode_frame, Frame, LinkConfig};
use iec102_core::transport::{FrameConnection, Parity, SerialSettings};
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;

/// 绑定一个随机端口并返回监听器与地址字符串。
async fn bind() -> (TcpListener, String) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    (listener, addr.to_string())
}

#[tokio::test]
async fn tcp_roundtrip_variable_frame() {
    let cfg = LinkConfig::new(1);
    let (listener, addr) = bind().await;

    let server = tokio::spawn(async move {
        let mut conn = FrameConnection::accept_tcp(&listener).await.unwrap();
        conn.read_frame(&cfg).await.unwrap()
    });

    let mut client = FrameConnection::connect_tcp(&addr).await.unwrap();
    let frame = Frame::Variable {
        control: 0x6B,
        address: 0x01,
        asdu: vec![0x64, 0x01, 0x00],
    };
    client.write_frame(&frame, &cfg).await.unwrap();

    let received = server.await.unwrap();
    assert_eq!(received, frame);
}

#[tokio::test]
async fn tcp_coalesced_frames() {
    let cfg = LinkConfig::new(1);
    let frames = vec![
        Frame::Fixed {
            control: 0x40,
            address: 0x01,
        },
        Frame::Variable {
            control: 0x6B,
            address: 0x02,
            asdu: vec![0x01],
        },
        Frame::Ack,
    ];
    let mut bytes = Vec::new();
    for frame in &frames {
        bytes.extend_from_slice(&encode_frame(frame, &cfg).unwrap());
    }

    let (listener, addr) = bind().await;
    let count = frames.len();
    let server = tokio::spawn(async move {
        let mut conn = FrameConnection::accept_tcp(&listener).await.unwrap();
        let mut got = Vec::new();
        for _ in 0..count {
            got.push(conn.read_frame(&cfg).await.unwrap());
        }
        got
    });

    let mut client = FrameConnection::connect_tcp(&addr).await.unwrap();
    client.write_bytes(&bytes).await.unwrap();

    let got = server.await.unwrap();
    assert_eq!(got, frames);
}

#[tokio::test]
async fn tcp_split_frame() {
    let cfg = LinkConfig::new(1);
    let frame = Frame::Variable {
        control: 0x6B,
        address: 0x01,
        asdu: vec![0x01, 0x02, 0x03, 0x04],
    };
    let encoded = encode_frame(&frame, &cfg).unwrap();
    let split = encoded.len() / 2;

    let (listener, addr) = bind().await;
    let server = tokio::spawn(async move {
        let mut conn = FrameConnection::accept_tcp(&listener).await.unwrap();
        conn.read_frame(&cfg).await.unwrap()
    });

    let mut client = FrameConnection::connect_tcp(&addr).await.unwrap();
    client.write_bytes(&encoded[..split]).await.unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;
    client.write_bytes(&encoded[split..]).await.unwrap();

    let received = server.await.unwrap();
    assert_eq!(received, frame);
}

#[tokio::test]
async fn tcp_garbage_prefix_is_skipped() {
    let cfg = LinkConfig::new(1);
    let (listener, addr) = bind().await;

    let server = tokio::spawn(async move {
        let mut conn = FrameConnection::accept_tcp(&listener).await.unwrap();
        let frame = conn.read_frame(&cfg).await.unwrap();
        (frame, conn.stats())
    });

    let mut client = FrameConnection::connect_tcp(&addr).await.unwrap();
    // `0x00` 与 `0xFF` 都不是合法启动字节，`0x68` 是可变帧启动字节。
    client.write_bytes(&[0x00, 0xFF, 0x68]).await.unwrap();
    let frame = Frame::Variable {
        control: 0x6B,
        address: 0x01,
        asdu: vec![0x01],
    };
    client.write_frame(&frame, &cfg).await.unwrap();

    let (received, stats) = server.await.unwrap();
    assert_eq!(received, frame);
    assert!(stats.parse_errors > 0, "非法前缀应计入 `parse_errors`");
}

#[tokio::test]
async fn tcp_bad_checksum_is_skipped() {
    let cfg = LinkConfig::new(1);
    // `0x41` 是 `[0x40, 0x01]` 的校验和，加 1 得到 `0x42`，不是任何启动字节。
    let bad = vec![0x10, 0x40, 0x01, 0x42, 0x16];
    let valid = Frame::Variable {
        control: 0x6B,
        address: 0x02,
        asdu: vec![0x01, 0x02],
    };

    let (listener, addr) = bind().await;
    let server = tokio::spawn(async move {
        let mut conn = FrameConnection::accept_tcp(&listener).await.unwrap();
        let frame = conn.read_frame(&cfg).await.unwrap();
        (frame, conn.stats())
    });

    let mut client = FrameConnection::connect_tcp(&addr).await.unwrap();
    client.write_bytes(&bad).await.unwrap();
    client.write_frame(&valid, &cfg).await.unwrap();

    let (received, stats) = server.await.unwrap();
    assert_eq!(received, valid);
    assert!(
        stats.checksum_errors >= 1,
        "校验和错误应计入 `checksum_errors`"
    );
}

#[tokio::test]
async fn write_frame_matches_encode_frame() {
    let cfg = LinkConfig::new(2);
    let frame = Frame::Variable {
        control: 0x6B,
        address: 0xBEEF,
        asdu: vec![0x01, 0x02, 0x03],
    };
    let expected = encode_frame(&frame, &cfg).unwrap();

    let (listener, addr) = bind().await;
    let expected_len = expected.len();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut buf = vec![0u8; expected_len];
        stream.read_exact(&mut buf).await.unwrap();
        buf
    });

    let mut client = FrameConnection::connect_tcp(&addr).await.unwrap();
    client.write_frame(&frame, &cfg).await.unwrap();

    let received = server.await.unwrap();
    assert_eq!(received, expected);
}

#[tokio::test]
async fn peer_is_non_empty() {
    let (listener, addr) = bind().await;

    let server = tokio::spawn(async move {
        let conn = FrameConnection::accept_tcp(&listener).await.unwrap();
        conn.peer()
    });

    let client = FrameConnection::connect_tcp(&addr).await.unwrap();
    assert!(!client.peer().is_empty());
    assert!(client.peer().starts_with("tcp "));

    let server_peer = server.await.unwrap();
    assert!(!server_peer.is_empty());
    assert!(server_peer.starts_with("tcp "));
}

#[tokio::test]
#[ignore = "需要真实串口或伪终端，默认不执行"]
async fn serial_roundtrip_over_pty() {
    // 需要真实串口或 `pty`（伪终端）。`macOS` 上可用
    // `socat -d -d pty,raw,echo=0 pty,raw,echo=0`
    // 创建一对伪终端，把其中一个路径填入 `port` 字段。
    let settings = SerialSettings {
        port: "/dev/tty.usbserial".to_string(),
        ..Default::default()
    };
    let mut conn = FrameConnection::open_serial(&settings).await.unwrap();
    assert!(!conn.peer().is_empty());

    let cfg = LinkConfig::new(1);
    let frame = Frame::Fixed {
        control: 0x40,
        address: 0x01,
    };
    conn.write_frame(&frame, &cfg).await.unwrap();
    let received = conn.read_frame(&cfg).await.unwrap();
    assert_eq!(received, frame);

    // 校验 `Parity` 默认值为偶校验。
    assert_eq!(SerialSettings::default().parity, Parity::Even);
}
