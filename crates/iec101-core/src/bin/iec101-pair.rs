//! 命令行主从对，用于在终端面板里并行观察 101 收发。
//!
//! 子站：`iec101-pair slave --bind 127.0.0.1:2401`
//! 主站：`iec101-pair master --connect 127.0.0.1:2401 --interrogate 20 --control 1:off`

use std::time::Duration;

use iec101_core::asdu::AsduAddress;
use iec101_core::ft12::{encode_frame, Frame, LinkConfig};
use iec101_core::link::LinkParams;
use iec101_core::session::{
    MasterStation, PointKind, PointValue, SlaveStation, StationConfig, StationEvent,
};
use iec101_core::transport::FrameConnection;

fn link_config(address_len: u8) -> LinkConfig {
    LinkConfig::new(address_len)
}

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
    let mut bind = "127.0.0.1:2401".to_string();
    let mut connect = "127.0.0.1:2401".to_string();
    let mut interrogate: Option<u8> = None;
    let mut control: Option<(u32, bool)> = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--bind" => bind = args.next().unwrap_or(bind),
            "--connect" => connect = args.next().unwrap_or(connect),
            "--interrogate" => interrogate = args.next().and_then(|v| v.parse().ok()),
            "--control" => {
                if let Some(value) = args.next() {
                    let mut parts = value.split(':');
                    let ioa = parts.next().and_then(|v| v.parse().ok());
                    let on = parts.next().map(|v| v == "on");
                    if let (Some(ioa), Some(on)) = (ioa, on) {
                        control = Some((ioa, on));
                    }
                }
            }
            _ => {}
        }
    }

    let cfg = link_config(1);
    let config = StationConfig {
        link: LinkParams {
            address: 1,
            address_len: 1,
            max_retries: 3,
        },
        address: AsduAddress {
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
        let mut station = SlaveStation::new(
            config,
            vec![
                (1, PointKind::SinglePoint),
                (2, PointKind::DoublePoint),
                (3, PointKind::ShortFloat),
            ],
        );
        station.set_value(1, PointValue::Bool(true));
        station.set_value(2, PointValue::Double(1));
        station.set_value(3, PointValue::Float(12.5));
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
                match event {
                    StationEvent::InterrogationStarted { qoi } => {
                        println!("[子站] 总召唤开始 qoi={qoi}")
                    }
                    StationEvent::InterrogationFinished => println!("[子站] 总召唤结束"),
                    StationEvent::ControlConfirmed { ioa, on } => {
                        println!("[子站] 遥控确认 ioa={ioa} on={on}")
                    }
                    StationEvent::ClockSynced => println!("[子站] 时钟已同步"),
                    StationEvent::PointChanged { ioa } => println!("[子站] 点变化 ioa={ioa}"),
                    StationEvent::LinkReset => println!("[子站] 链路复位"),
                    StationEvent::ProcessReset => println!("[子站] 进程复位"),
                    StationEvent::CommonAddressMismatch { expected, actual } => {
                        println!("[子站] 公共地址不匹配: 期望 {expected}, 实际 {actual}")
                    }
                }
            }
        }
        return Ok(());
    }

    let mut connection = FrameConnection::connect_tcp(&connect).await?;
    println!("[主站] 已连接 {connect}");
    let mut station = MasterStation::new(config);
    let mut ticker = tokio::time::interval(Duration::from_millis(200));
    let mut sent_interrogate = false;
    let mut sent_control = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        if tokio::time::Instant::now() >= deadline {
            break;
        }
        tokio::select! {
            _ = ticker.tick() => {
                // 先推进超时计时,到点时重发上一帧;否则按轮询节奏发下一帧。
                let frame = match station.tick(200) {
                    Some(frame) => Some(frame),
                    None => station.next_frame(),
                };
                if let Some(frame) = frame {
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
        if !sent_interrogate {
            if let Some(qoi) = interrogate {
                if station.interrogate(qoi) {
                    println!("[主站] 发起总召唤 qoi={qoi}");
                    sent_interrogate = true;
                }
            }
        }
        if let Some((ioa, on)) = control {
            if !sent_control && station.control(ioa, on) {
                println!("[主站] 发起遥控 ioa={ioa} on={on}");
                sent_control = true;
            }
        }
    }
    let mut points = station.received();
    points.sort_by_key(|(ioa, _)| *ioa);
    println!("[主站] 收到的数据 {points:?}");
    Ok(())
}
