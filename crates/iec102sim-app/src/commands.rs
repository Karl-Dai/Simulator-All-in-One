use crate::state::{
    AppState, CounterView, LogEntry, SharedState, SlaveCommand, SlaveSnapshot, StatsView,
    MAX_LOGS,
};
use iec102_core::asdu::{decode_asdu102, StationAddress};
use iec102_core::ft12::{encode_frame, ControlField, Frame, LinkConfig};
use iec102_core::link::{LinkParams, LinkState};
use iec102_core::session::{
    CounterPoint, SlaveStation102, StationConfig102, StationEvent102,
};
use iec102_core::transport::{FrameConnection, Parity, SerialSettings};
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
pub struct CounterInit {
    pub ioa: u32,
    pub reading: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlaveStartConfig {
    pub transport: TransportConfig,
    pub link: LinkConfigDto,
    pub counters: Vec<CounterInit>,
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

    let points = config
        .counters
        .iter()
        .map(|counter| CounterPoint {
            ioa: counter.ioa,
            reading: counter.reading,
        })
        .collect();

    let station_config = StationConfig102 {
        link: LinkParams {
            address: config.link.address,
            address_len: config.link.address_len,
            max_retries: 3,
        },
        address: StationAddress {
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
        guard.counters = config
            .counters
            .iter()
            .map(|counter| CounterView {
                ioa: counter.ioa,
                reading: counter.reading,
                sequence: 0,
            })
            .collect();
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
        points,
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
pub async fn set_reading(
    state: State<'_, AppState>,
    ioa: u32,
    reading: u32,
) -> Result<(), String> {
    let tx = if let Ok(guard) = state.shared.lock() {
        guard.command_tx.clone()
    } else {
        None
    };
    let tx = tx.ok_or_else(|| "子站未运行".to_string())?;
    tx.send(SlaveCommand::SetReading { ioa, reading })
        .await
        .map_err(|_| "子站已停止".to_string())?;
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
            counters: Vec::new(),
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
    station_config: StationConfig102,
    points: Vec<CounterPoint>,
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

    let mut station = SlaveStation102::new(station_config, points);

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
                    Some(SlaveCommand::SetReading { ioa, reading }) => {
                        if station.set_reading(ioa, reading) {
                            push_log(
                                &app,
                                &shared,
                                LogEntry::new(
                                    "info",
                                    format!("设置 IOA={ioa} 读数={reading}"),
                                ),
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

/// 处理子站事件，写日志并把读数同步到点表视图。
fn handle_station_events(
    app: &AppHandle,
    shared: &Arc<Mutex<SharedState>>,
    station: &mut SlaveStation102,
) {
    for event in station.drain_events() {
        match event {
            StationEvent102::LinkReset => {
                push_log(app, shared, LogEntry::new("info", "链路复位"));
            }
            StationEvent102::InterrogationStarted { qcc } => {
                push_log(
                    app,
                    shared,
                    LogEntry::new(
                        "info",
                        format!(
                            "计量召唤开始 请求分组={} 冻结方式={}",
                            qcc.request, qcc.freeze
                        ),
                    ),
                );
            }
            StationEvent102::InterrogationFinished => {
                push_log(app, shared, LogEntry::new("info", "计量召唤结束"));
            }
            StationEvent102::CounterFrozen { ioa, reading } => {
                push_log(
                    app,
                    shared,
                    LogEntry::new("info", format!("冻结 IOA={ioa} 读数={reading}")),
                );
            }
            StationEvent102::CounterReset { ioa } => {
                push_log(app, shared, LogEntry::new("info", format!("复位 IOA={ioa}")));
            }
            StationEvent102::CommonAddressMismatch { expected, actual } => {
                push_log(
                    app,
                    shared,
                    LogEntry::new(
                        "error",
                        format!("公共地址不匹配: 期望 {expected}, 实际 {actual}"),
                    ),
                );
            }
        }
    }
    sync_counters(shared, station);
}

/// 把子站当前读数与顺序号同步到点表视图。
fn sync_counters(shared: &Arc<Mutex<SharedState>>, station: &SlaveStation102) {
    if let Ok(mut guard) = shared.lock() {
        for counter in guard.counters.iter_mut() {
            if let Some(reading) = station.reading(counter.ioa) {
                counter.reading = reading;
            }
            if let Some(sequence) = station.sequence(counter.ioa) {
                counter.sequence = sequence;
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
    station_config: &StationConfig102,
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

fn describe_frame(frame: &Frame, direction: &str, station_config: &StationConfig102) -> String {
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

fn describe_asdu(bytes: &[u8], address: StationAddress) -> Option<String> {
    let (asdu, _) = decode_asdu102(bytes, address).ok()?;
    let type_name = asdu_type_name(asdu.type_id)?;
    let cause = cause_short(asdu.cause);
    Some(format!(
        "{type_name}{cause} CA={}",
        asdu.address.common_address
    ))
}

fn asdu_type_name(type_id: u8) -> Option<&'static str> {
    Some(match type_id {
        100 => "累计量",
        101 => "带 CP24Time2a 累计量",
        103 => "带 CP56Time2a 累计量",
        120 => "累计量召唤",
        121 => "带 CP24Time2a 累计量召唤",
        123 => "带 CP56Time2a 累计量召唤",
        _ => return None,
    })
}

fn cause_short(cause: u8) -> String {
    match cause {
        6 => "激活".to_string(),
        7 => "激活确认".to_string(),
        10 => "激活终止".to_string(),
        37 => "请求计数量".to_string(),
        45 => "否定确认".to_string(),
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

/// 恢复界面配置；首次启动把旧 WebView 存储迁移进 SQLite。
#[tauri::command]
pub async fn simlab_load_preferences(
    state: State<'_, AppState>,
    legacy: serde_json::Value,
) -> Result<serde_json::Value, String> {
    state.suite_storage.load_preferences(legacy)
}

/// 保存界面配置到 SQLite，供重启后恢复。
#[tauri::command]
pub async fn simlab_save_preferences(
    state: State<'_, AppState>,
    values: serde_json::Value,
) -> Result<(), String> {
    state.suite_storage.save_preferences(values)
}
