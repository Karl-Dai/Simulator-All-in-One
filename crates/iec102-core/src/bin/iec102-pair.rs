//! 命令行主从对，用于在终端面板里并行观察 102 收发。
//!
//! 子站：`iec102-pair slave --bind 127.0.0.1:2405`
//! 主站：`iec102-pair master --connect 127.0.0.1:2405 --interrogate 0:1`

use std::time::Duration;

use iec102_core::asdu::{Qcc, StationAddress};
use iec102_core::ft12::{encode_frame, Frame, LinkConfig};
use iec102_core::link::LinkParams;
use iec102_core::session::{CounterPoint, MasterStation102, SlaveStation102, StationConfig102};
use iec102_core::transport::FrameConnection;

fn describe(frame: &Frame) -> String {
    match frame {
        Frame::Ack => "肯定认可".to_string(),
        Frame::Fixed { control, address } => {
            format!("固定帧 control=0x{control:02X} address={address}")
        }
        Frame::Variable {
            control,
            address,
            asdu,
        } => format!(
            "可变帧 control=0x{control:02X} address={address} type={} len={}",
            asdu.first().copied().unwrap_or(0),
            asdu.len()
        ),
    }
}

fn hex(frame: &Frame, config: LinkConfig) -> String {
    encode_frame(frame, &config)
        .map(|bytes| bytes.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" "))
        .unwrap_or_default()
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let mut args = std::env::args().skip(1);
    let role = args.next().unwrap_or_else(|| "slave".to_string());
    let mut bind = "127.0.0.1:2405".to_string();
    let mut connect = "127.0.0.1:2405".to_string();
    let mut interrogate: Option<(u8, u8)> = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--bind" => bind = args.next().unwrap_or(bind),
            "--connect" => connect = args.next().unwrap_or(connect),
            "--interrogate" => {
                if let Some(value) = args.next() {
                    let mut parts = value.split(':');
                    let request = parts.next().and_then(|v| v.parse().ok());
                    let freeze = parts.next().and_then(|v| v.parse().ok());
                    if let (Some(request), Some(freeze)) = (request, freeze) {
                        interrogate = Some((request, freeze));
                    }
                }
            }
            _ => {}
        }
    }

    let cfg = LinkConfig::new(1);
    let config = StationConfig102 {
        link: LinkParams {
            address: 1,
            address_len: 1,
            max_retries: 3,
        },
        address: StationAddress {
            common_address: 1,
            ca_len: 1,
            ioa_len: 2,
        },
        poll_interval_ms: 200,
        response_timeout_ms: 3000,
    };

    if role == "slave" {
        let listener = tokio::net::TcpListener::bind(&bind).await?;
        println!("[子站] 监听 {bind}");
        let mut connection = FrameConnection::accept_tcp(&listener).await?;
        println!("[子站] 主站已连接");
        let mut station = SlaveStation102::new(
            config,
            vec![
                CounterPoint { ioa: 1, reading: 100 },
                CounterPoint { ioa: 2, reading: 200 },
                CounterPoint { ioa: 3, reading: 300 },
            ],
        );
        loop {
            let frame = match connection.read_frame(&cfg).await {
                Ok(frame) => frame,
                Err(error) => {
                    println!("[子站] 连接结束: {error}");
                    break;
                }
            };
            println!("[子站] RX {} | {}", hex(&frame, cfg), describe(&frame));
            if let Some(reply) = station.handle_frame(&frame) {
                println!("[子站] TX {} | {}", hex(&reply, cfg), describe(&reply));
                connection.write_frame(&reply, &cfg).await?;
            }
            for event in station.drain_events() {
                println!("[子站] 事件 {event:?}");
            }
        }
        return Ok(());
    }

    let mut connection = FrameConnection::connect_tcp(&connect).await?;
    println!("[主站] 已连接 {connect}");
    let mut station = MasterStation102::new(config);
    let mut ticker = tokio::time::interval(Duration::from_millis(200));
    let mut sent = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        if tokio::time::Instant::now() >= deadline {
            break;
        }
        tokio::select! {
            _ = ticker.tick() => {
                if let Some(frame) = station.next_frame() {
                    println!("[主站] TX {} | {}", hex(&frame, cfg), describe(&frame));
                    connection.write_frame(&frame, &cfg).await?;
                }
            }
            result = connection.read_frame(&cfg) => {
                match result {
                    Ok(frame) => {
                        println!("[主站] RX {} | {}", hex(&frame, cfg), describe(&frame));
                        station.handle_frame(&frame);
                    }
                    Err(error) => { println!("[主站] 连接结束: {error}"); break; }
                }
            }
        }
        for event in station.drain_events() {
            println!("[主站] 事件 {event:?}");
        }
        if let Some((request, freeze)) = interrogate {
            if !sent && station.interrogate(Qcc { request, freeze }) {
                println!("[主站] 发起计量召唤 request={request} freeze={freeze}");
                sent = true;
            }
        }
    }
    let mut counters = station.received();
    counters.sort_by_key(|(ioa, _, _)| *ioa);
    println!("[主站] 收到的计量点 {counters:?}");
    Ok(())
}
