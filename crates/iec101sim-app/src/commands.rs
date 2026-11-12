use crate::state::{
    AppState, LogEntry, PointView, SharedState, SlaveSnapshot, StatsView, MAX_LOGS,
};
use iec101_core::asdu::{decode_asdu, AsduAddress};
use iec101_core::ft12::{encode_frame, ControlField, Frame, LinkConfig};
use iec101_core::link::{LinkParams, LinkState};
use iec101_core::session::{
    PointKind, PointValue, SlaveCommand, SlaveStation, StationConfig, StationEvent,
};
use iec101_core::transport::{FrameConnection, Parity, SerialSettings};
use serde::Deserialize;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, State};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

// ---------------------------------------------------------------------------
// DTOs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransportConfig {
    /// "tcp" 或 "serial"。
    pub mode: String,
    pub host: String,
    pub port: u16,
    pub serial_port: String,
    pub baud_rate: u32,
    pub data_bits: u8,
    /// "none" | "odd" | "even"。
    pub parity: String,
    pub stop_bits: u8,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkConfigDto {
    pub address: u16,
    pub address_len: u8,
    pub common_address: u16,
    pub ca_len: u8,
    pub ioa_len: u8,
    pub poll_interval_ms: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PointInit {
    pub ioa: u32,
    /// "single" | "double" | "normalized" | "scaled" | "float"。
    pub kind: String,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlaveStartConfig {
    pub transport: TransportConfig,
    pub link: LinkConfigDto,
    pub points: Vec<PointInit>,
}

/// 连接来源：TCP 已绑定的监听器，或已打开的串口连接。
enum ConnectionSource {
    Tcp(TcpListener),
    Ready(FrameConnection),
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn start(
    app: AppHandle,
    state: State<'_, AppState>,
    config: SlaveStartConfig,
) -> Result<(), String> {
    if let Ok(guard) = state.shared.lock() {
        if guard.running {
            return Err("子站已在运行，请先停止".to_string());
        }
    }

    let mut point_kinds = Vec::with_capacity(config.points.len());
    let mut initial_values = Vec::with_capacity(config.points.len());
    let mut point_views = Vec::with_capacity(config.points.len());
    for point in &config.points {
        let kind = parse_point_kind(&point.kind)
            .ok_or_else(|| format!("IOA={} 不支持的点位类型: {}", point.ioa, point.kind))?;
        let value = json_to_point_value(kind, &point.value)
            .ok_or_else(|| format!("IOA={} 点位值与类型 {} 不匹配", point.ioa, point.kind))?;
        point_kinds.push((point.ioa, kind));
        initial_values.push((point.ioa, value));
        point_views.push(PointView {
            ioa: point.ioa,
            kind: point.kind.clone(),
            value: point.value.clone(),
            quality: String::new(),
        });
    }

    let station_config = StationConfig {
        link: LinkParams {
            address: config.link.address,
            address_len: config.link.address_len,
            max_retries: 3,
        },
        address: AsduAddress {
            common_address: config.link.common_address,
            ca_len: config.link.ca_len,
            ioa_len: config.link.ioa_len,
        },
        poll_interval_ms: config.link.poll_interval_ms,
        response_timeout_ms: 3000,
    };

    let source = open_connection_source(&config.transport).await?;
    let (tx, rx) = mpsc::channel::<SlaveCommand>(64);

    if let Ok(mut guard) = state.shared.lock() {
        guard.running = true;
        guard.link_state = "reset".to_string();
        guard.points = point_views;
        guard.stats = StatsView::default();
        guard.command_tx = Some(tx);
    }

    push_log(
        &app,
        &state.shared,
        LogEntry::new(
            "info",
            format!("已启动 {}", describe_transport(&config.transport)),
        ),
    );

    let shared = state.shared.clone();
    let handle = state.runtime.spawn(run_loop(
        app.clone(),
        shared,
        source,
        station_config,
        point_kinds,
        initial_values,
        rx,
    ));
    if let Ok(mut guard) = state.task.lock() {
        *guard = Some(handle);
    }
    Ok(())
}

#[tauri::command]
pub async fn stop(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let task = if let Ok(mut guard) = state.task.lock() {
        guard.take()
    } else {
        None
    };
    if let Some(task) = task {
        task.abort();
    }
    reset_state(&state.shared);
    push_log(&app, &state.shared, LogEntry::new("info", "子站已停止"));
    emit_snapshot(&app, &state.shared);
    Ok(())
}

#[tauri::command]
pub async fn set_point(
    app: AppHandle,
    state: State<'_, AppState>,
    ioa: u32,
    value: serde_json::Value,
) -> Result<(), String> {
    let (tx, kind) = if let Ok(guard) = state.shared.lock() {
        (
            guard.command_tx.clone(),
            guard.points
                .iter()
                .find(|point| point.ioa == ioa)
                .map(|point| point.kind.clone()),
        )
    } else {
        (None, None)
    };
    let tx = tx.ok_or_else(|| "子站未运行".to_string())?;
    let kind = kind.ok_or_else(|| format!("IOA={ioa} 点位不存在"))?;
    let point_kind =
        parse_point_kind(&kind).ok_or_else(|| format!("IOA={ioa} 点位类型未知: {kind}"))?;
    let point_value = json_to_point_value(point_kind, &value)
        .ok_or_else(|| format!("IOA={ioa} 点位值与类型 {kind} 不匹配"))?;

    tx.send(SlaveCommand::SetValue {
        ioa,
        value: point_value,
    })
    .await
    .map_err(|_| "子站已停止".to_string())?;

    if let Ok(mut guard) = state.shared.lock() {
        if let Some(point) = guard.points.iter_mut().find(|point| point.ioa == ioa) {
            point.value = point_value_to_json(&point_value);
        }
    }
    emit_snapshot(&app, &state.shared);
    Ok(())
}

#[tauri::command]
pub fn snapshot(state: State<'_, AppState>) -> SlaveSnapshot {
    if let Ok(guard) = state.shared.lock() {
        guard.snapshot()
    } else {
        SlaveSnapshot {
            running: false,
            link_state: "idle".to_string(),
            points: Vec::new(),
            stats: StatsView::default(),
            logs: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Connection loop
// ---------------------------------------------------------------------------

async fn run_loop(
    app: AppHandle,
    shared: Arc<Mutex<SharedState>>,
    source: ConnectionSource,
    station_config: StationConfig,
    point_kinds: Vec<(u32, PointKind)>,
    initial_values: Vec<(u32, PointValue)>,
    mut commands: mpsc::Receiver<SlaveCommand>,
) {
    let mut connection = match open_connection(source).await {
        Ok(connection) => connection,
        Err(err) => {
            push_log(
                &app,
                &shared,
                LogEntry::new("error", format!("连接失败: {err}")),
            );
            reset_state(&shared);
            emit_snapshot(&app, &shared);
            return;
        }
    };

    let mut station = SlaveStation::new(station_config, point_kinds);
    for (ioa, value) in initial_values {
        if !station.set_value(ioa, value) {
            push_log(
                &app,
                &shared,
                LogEntry::new("error", format!("IOA={ioa} 初始值设置失败")),
            );
        }
    }

    let link_config = LinkConfig::new(station_config.link.address_len);
    let mut timer = tokio::time::interval(std::time::Duration::from_millis(50));
    let mut link_state = LinkState::Reset;

    loop {
        tokio::select! {
            result = connection.read_frame(&link_config) => {
                match result {
                    Ok(frame) => {
                        if let Some(control) = frame_control(&frame) {
                            let cf = ControlField::parse(control);
                            if cf.prm && cf.function == 0 {
                                link_state = LinkState::Available;
                            }
                        }
                        push_log(
                            &app,
                            &shared,
                            frame_log_entry("rx", &frame, &link_config, &station_config),
                        );
                        if let Some(reply) = station.handle_frame(&frame) {
                            push_log(
                                &app,
                                &shared,
                                frame_log_entry("tx", &reply, &link_config, &station_config),
                            );
                            if let Err(err) = connection.write_frame(&reply, &link_config).await {
                                push_log(
                                    &app,
                                    &shared,
                                    LogEntry::new("error", format!("写帧失败: {err}")),
                                );
                                break;
                            }
                        }
                        handle_station_events(&app, &shared, &mut station);
                    }
                    Err(err) => {
                        push_log(
                            &app,
                            &shared,
                            LogEntry::new("error", format!("读取帧失败: {err}")),
                        );
                        break;
                    }
                }
            }
            command = commands.recv() => {
                match command {
                    Some(SlaveCommand::SetValue { ioa, value }) => {
                        if station.set_value(ioa, value) {
                            let json = point_value_to_json(&value);
                            if let Ok(mut guard) = shared.lock() {
                                if let Some(point) = guard.points.iter_mut().find(|p| p.ioa == ioa) {
                                    point.value = json.clone();
                                }
                            }
                            push_log(
                                &app,
                                &shared,
                                LogEntry::new("info", format!("设置 IOA={ioa} 值={json}")),
                            );
                        } else {
                            push_log(
                                &app,
                                &shared,
                                LogEntry::new("error", format!("设置 IOA={ioa} 失败")),
                            );
                        }
                    }
                    None => break,
                }
                handle_station_events(&app, &shared, &mut station);
            }
            _ = timer.tick() => {
                handle_station_events(&app, &shared, &mut station);
            }
        }

        let stats = station.stats();
        let snapshot = if let Ok(mut guard) = shared.lock() {
            guard.link_state = if guard.running {
                link_state_str(link_state).to_string()
            } else {
                "idle".to_string()
            };
            guard.stats = StatsView {
                frames_rx: stats.frames_rx,
                frames_tx: stats.frames_tx,
                errors: stats.errors,
            };
            guard.snapshot()
        } else {
            continue;
        };
        let _ = app.emit("snapshot", &snapshot);
    }

    reset_state(&shared);
    emit_snapshot(&app, &shared);
}

/// 处理子站事件，更新点表并写日志。
fn handle_station_events(
    app: &AppHandle,
    shared: &Arc<Mutex<SharedState>>,
    station: &mut SlaveStation,
) {
    for event in station.drain_events() {
        match event {
            StationEvent::LinkReset => {
                push_log(app, shared, LogEntry::new("info", "链路复位"));
            }
            StationEvent::InterrogationStarted { qoi } => {
                push_log(
                    app,
                    shared,
                    LogEntry::new("info", format!("总召唤开始 QOI={qoi}")),
                );
            }
            StationEvent::InterrogationFinished => {
                push_log(app, shared, LogEntry::new("info", "总召唤结束"));
            }
            StationEvent::PointChanged { ioa } => {
                if let Some(value) = station.value(ioa) {
                    let json = point_value_to_json(&value);
                    if let Ok(mut guard) = shared.lock() {
                        if let Some(point) = guard.points.iter_mut().find(|p| p.ioa == ioa) {
                            point.value = json.clone();
                        }
                    }
                    push_log(
                        app,
                        shared,
                        LogEntry::new("info", format!("更新 IOA={ioa} 值={json}")),
                    );
                }
            }
            StationEvent::ControlConfirmed { ioa, on } => {
                push_log(
                    app,
                    shared,
                    LogEntry::new(
                        "info",
                        format!("遥控确认 IOA={ioa} {}", if on { "合" } else { "分" }),
                    ),
                );
            }
            StationEvent::ClockSynced => {
                push_log(app, shared, LogEntry::new("info", "时钟同步完成"));
            }
            StationEvent::ProcessReset => {
                push_log(app, shared, LogEntry::new("info", "进程复位"));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async fn open_connection_source(transport: &TransportConfig) -> Result<ConnectionSource, String> {
    match transport.mode.as_str() {
        "tcp" => {
            let address = format!("{}:{}", transport.host, transport.port);
            let listener = TcpListener::bind(&address)
                .await
                .map_err(|err| format!("TCP 监听失败: {err}"))?;
            Ok(ConnectionSource::Tcp(listener))
        }
        "serial" => {
            let settings = SerialSettings {
                port: transport.serial_port.clone(),
                baud_rate: transport.baud_rate,
                data_bits: transport.data_bits,
                parity: parse_parity(&transport.parity)?,
                stop_bits: transport.stop_bits,
            };
            let connection = FrameConnection::open_serial(&settings)
                .await
                .map_err(|err| format!("打开串口失败: {err}"))?;
            Ok(ConnectionSource::Ready(connection))
        }
        other => Err(format!("不支持的传输模式: {other}")),
    }
}

async fn open_connection(source: ConnectionSource) -> Result<FrameConnection, String> {
    match source {
        ConnectionSource::Tcp(listener) => {
            FrameConnection::accept_tcp(&listener)
                .await
                .map_err(|err| format!("接受 TCP 连接失败: {err}"))
        }
        ConnectionSource::Ready(connection) => Ok(connection),
    }
}

fn parse_parity(value: &str) -> Result<Parity, String> {
    match value {
        "none" => Ok(Parity::None),
        "odd" => Ok(Parity::Odd),
        "even" => Ok(Parity::Even),
        other => Err(format!("不支持的校验方式: {other}")),
    }
}

fn describe_transport(transport: &TransportConfig) -> String {
    if transport.mode == "tcp" {
        format!("tcp://{}:{}", transport.host, transport.port)
    } else {
        format!(
            "serial://{}@{}bps",
            transport.serial_port, transport.baud_rate
        )
    }
}

fn frame_control(frame: &Frame) -> Option<u8> {
    match frame {
        Frame::Fixed { control, .. } => Some(*control),
        Frame::Variable { control, .. } => Some(*control),
        Frame::Ack => None,
    }
}

fn frame_log_entry(
    direction: &str,
    frame: &Frame,
    link_config: &LinkConfig,
    station_config: &StationConfig,
) -> LogEntry {
    let hex = match encode_frame(frame, link_config) {
        Ok(bytes) => bytes
            .iter()
            .map(|byte| format!("{byte:02X}"))
            .collect::<Vec<_>>()
            .join(" "),
        Err(_) => String::new(),
    };
    let summary = describe_frame(frame, direction, station_config);
    LogEntry::with_hex(direction, summary, hex)
}

fn describe_frame(frame: &Frame, direction: &str, station_config: &StationConfig) -> String {
    let prefix = if direction == "tx" { "TX" } else { "RX" };
    match frame {
        Frame::Ack => format!("{prefix} 肯定认可"),
        Frame::Fixed { control, address } => {
            let cf = ControlField::parse(*control);
            let action = if cf.prm {
                match cf.function {
                    0 => "复位链路",
                    1 => "复位 FCB",
                    9 => "请求链路状态",
                    10 => "请求一级数据",
                    11 => "请求二级数据",
                    other => return format!("{prefix} 未知主站功能码 {other} addr={address}"),
                }
            } else {
                match cf.function {
                    0 => "肯定认可",
                    1 => "否定认可",
                    11 => "链路忙",
                    other => return format!("{prefix} 未知子站功能码 {other} addr={address}"),
                }
            };
            format!("{prefix} {action} addr={address}")
        }
        Frame::Variable {
            control,
            address,
            asdu,
        } => {
            let cf = ControlField::parse(*control);
            let action = if cf.prm {
                match cf.function {
                    2 => "发送/确认用户数据",
                    4 => "发送/无回答用户数据",
                    other => return format!("{prefix} 未知主站功能码 {other} addr={address}"),
                }
            } else {
                match cf.function {
                    8 => "用户数据",
                    other => return format!("{prefix} 未知子站功能码 {other} addr={address}"),
                }
            };
            match describe_asdu(asdu, station_config.address) {
                Some(desc) => format!("{prefix} {desc}"),
                None => format!("{prefix} {action} addr={address}"),
            }
        }
    }
}

fn describe_asdu(bytes: &[u8], address: AsduAddress) -> Option<String> {
    let (asdu, _) = decode_asdu(bytes, address).ok()?;
    let type_name = asdu_type_name(asdu.type_id)?;
    let cause = cause_short(asdu.cause);
    Some(format!(
        "{type_name}{cause} CA={}",
        asdu.address.common_address
    ))
}

fn asdu_type_name(type_id: u8) -> Option<&'static str> {
    Some(match type_id {
        1 => "单点信息",
        3 => "双点信息",
        9 => "归一化测量值",
        11 => "标度化测量值",
        13 => "短浮点测量值",
        30 => "带时标单点信息",
        31 => "带时标双点信息",
        34 => "带时标归一化测量值",
        35 => "带时标标度化测量值",
        36 => "带时标短浮点测量值",
        45 => "单命令",
        46 => "双命令",
        47 => "步调节命令",
        48 => "归一化设点",
        49 => "标度化设点",
        50 => "短浮点设点",
        100 => "总召唤",
        101 => "计数量召唤",
        103 => "时钟同步",
        105 => "复位进程",
        _ => return None,
    })
}

fn cause_short(cause: u8) -> String {
    match cause {
        3 => "突发".to_string(),
        6 => "激活".to_string(),
        7 => "确认".to_string(),
        10 => "终止".to_string(),
        20 => "响应召唤".to_string(),
        45 => "否定确认".to_string(),
        47 => "未知 IOA".to_string(),
        other => format!("原因 {other}"),
    }
}

fn link_state_str(state: LinkState) -> &'static str {
    match state {
        LinkState::Reset => "reset",
        LinkState::Available => "available",
        LinkState::Busy => "busy",
    }
}

fn parse_point_kind(kind: &str) -> Option<PointKind> {
    match kind {
        "single" => Some(PointKind::SinglePoint),
        "double" => Some(PointKind::DoublePoint),
        "normalized" => Some(PointKind::Normalized),
        "scaled" => Some(PointKind::Scaled),
        "float" => Some(PointKind::ShortFloat),
        _ => None,
    }
}

fn json_to_point_value(kind: PointKind, value: &serde_json::Value) -> Option<PointValue> {
    match kind {
        PointKind::SinglePoint => value.as_bool().map(PointValue::Bool),
        PointKind::DoublePoint => value.as_u64().and_then(|value| {
            if value <= 3 {
                Some(PointValue::Double(value as u8))
            } else {
                None
            }
        }),
        PointKind::Normalized => value.as_i64().and_then(|value| {
            if value >= i16::MIN as i64 && value <= i16::MAX as i64 {
                Some(PointValue::Normalized(value as i16))
            } else {
                None
            }
        }),
        PointKind::Scaled => value.as_i64().and_then(|value| {
            if value >= i16::MIN as i64 && value <= i16::MAX as i64 {
                Some(PointValue::Scaled(value as i16))
            } else {
                None
            }
        }),
        PointKind::ShortFloat => value.as_f64().map(|value| PointValue::Float(value as f32)),
    }
}

fn point_value_to_json(value: &PointValue) -> serde_json::Value {
    match value {
        PointValue::Bool(value) => serde_json::Value::Bool(*value),
        PointValue::Double(value) => serde_json::Value::Number((*value).into()),
        PointValue::Normalized(value) => serde_json::Value::Number((*value).into()),
        PointValue::Scaled(value) => serde_json::Value::Number((*value).into()),
        PointValue::Float(value) => serde_json::Number::from_f64(*value as f64)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
    }
}

fn reset_state(shared: &Arc<Mutex<SharedState>>) {
    if let Ok(mut guard) = shared.lock() {
        guard.running = false;
        guard.link_state = "idle".to_string();
        guard.command_tx = None;
    }
}

fn emit_snapshot(app: &AppHandle, shared: &Arc<Mutex<SharedState>>) {
    if let Ok(guard) = shared.lock() {
        let _ = app.emit("snapshot", guard.snapshot());
    }
}

fn push_log(app: &AppHandle, shared: &Arc<Mutex<SharedState>>, entry: LogEntry) {
    if let Ok(mut guard) = shared.lock() {
        guard.logs.push_back(entry.clone());
        while guard.logs.len() > MAX_LOGS {
            guard.logs.pop_front();
        }
    }
    let _ = app.emit("log", &entry);
}
