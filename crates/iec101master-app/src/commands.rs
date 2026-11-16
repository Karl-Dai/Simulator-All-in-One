use crate::state::{
    AppState, LogEntry, MasterSnapshot, PointView, SharedState, StatsView, MAX_LOGS,
};
use chrono::{Datelike, Timelike};
use iec101_core::asdu::{decode_asdu, AsduAddress, Cp56Time2a};
use iec101_core::ft12::{encode_frame, ControlField, Frame, LinkConfig};
use iec101_core::link::{LinkParams, LinkState};
use iec101_core::session::{
    MasterEvent101, MasterRequest, MasterStation, PointValue, StationConfig,
};
use iec101_core::transport::{FrameConnection, Parity, SerialSettings};
use serde::Deserialize;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, State};
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
pub struct MasterConnectConfig {
    pub transport: TransportConfig,
    pub link: LinkConfigDto,
    /// 仅用于界面初始展示，主站不写入。
    pub points: Vec<PointInit>,
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn connect(
    app: AppHandle,
    state: State<'_, AppState>,
    config: MasterConnectConfig,
) -> Result<(), String> {
    if let Ok(guard) = state.shared.lock() {
        if guard.connected {
            return Err("已有连接，请先断开".to_string());
        }
    }

    let connection = open_connection(&config.transport).await?;
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
    let station = MasterStation::new(station_config);
    let (tx, rx) = mpsc::channel::<MasterRequest>(64);

    if let Ok(mut guard) = state.shared.lock() {
        guard.connected = true;
        guard.link_state = "reset".to_string();
        guard.points = config
            .points
            .iter()
            .map(|point| PointView {
                ioa: point.ioa,
                kind: point.kind.clone(),
                value: point.value.clone(),
                quality: String::new(),
            })
            .collect();
        guard.stats = StatsView::default();
        guard.request_tx = Some(tx);
    }

    push_log(
        &app,
        &state.shared,
        LogEntry::new(
            "info",
            format!("已连接 {}", describe_transport(&config.transport)),
        ),
    );

    let shared = state.shared.clone();
    let handle = state.runtime.spawn(run_loop(
        app.clone(),
        shared,
        connection,
        station,
        station_config,
        rx,
    ));
    if let Ok(mut guard) = state.task.lock() {
        *guard = Some(handle);
    }
    Ok(())
}

#[tauri::command]
pub async fn disconnect(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let task = if let Ok(mut guard) = state.task.lock() {
        guard.take()
    } else {
        None
    };
    if let Some(task) = task {
        task.abort();
    }
    if let Ok(mut guard) = state.shared.lock() {
        guard.connected = false;
        guard.link_state = "idle".to_string();
        guard.request_tx = None;
    }
    push_log(&app, &state.shared, LogEntry::new("info", "连接已断开"));
    Ok(())
}

#[tauri::command]
pub async fn interrogate(state: State<'_, AppState>, qoi: u8) -> Result<(), String> {
    let tx = if let Ok(guard) = state.shared.lock() {
        guard.request_tx.clone()
    } else {
        None
    };
    match tx {
        Some(tx) => tx
            .send(MasterRequest::Interrogate { qoi })
            .await
            .map_err(|_| "连接已断开".to_string()),
        None => Err("未连接".to_string()),
    }
}

#[tauri::command]
pub async fn control(state: State<'_, AppState>, ioa: u32, on: bool) -> Result<(), String> {
    let tx = if let Ok(guard) = state.shared.lock() {
        guard.request_tx.clone()
    } else {
        None
    };
    match tx {
        Some(tx) => tx
            .send(MasterRequest::Control { ioa, on })
            .await
            .map_err(|_| "连接已断开".to_string()),
        None => Err("未连接".to_string()),
    }
}

#[tauri::command]
pub async fn sync_clock(state: State<'_, AppState>) -> Result<(), String> {
    let now = chrono::Local::now();
    let time = Cp56Time2a {
        milliseconds: now.timestamp_subsec_millis() as u16,
        minute: now.minute() as u8,
        hour: now.hour() as u8,
        day: now.day() as u8,
        month: now.month() as u8,
        year: now.year() as u16,
        invalid: false,
        summer_time: false,
    };
    let tx = if let Ok(guard) = state.shared.lock() {
        guard.request_tx.clone()
    } else {
        None
    };
    match tx {
        Some(tx) => tx
            .send(MasterRequest::SyncClock { time })
            .await
            .map_err(|_| "连接已断开".to_string()),
        None => Err("未连接".to_string()),
    }
}

#[tauri::command]
pub fn snapshot(state: State<'_, AppState>) -> MasterSnapshot {
    if let Ok(guard) = state.shared.lock() {
        guard.snapshot()
    } else {
        MasterSnapshot {
            connected: false,
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
    mut connection: FrameConnection,
    mut station: MasterStation,
    station_config: StationConfig,
    mut requests: mpsc::Receiver<MasterRequest>,
) {
    let link_config = LinkConfig::new(station_config.link.address_len);
    let interval_ms = station_config.poll_interval_ms.max(1);
    let mut timer = tokio::time::interval(std::time::Duration::from_millis(interval_ms));
    let mut link_state = LinkState::Reset;

    loop {
        tokio::select! {
            result = connection.read_frame(&link_config) => {
                match result {
                    Ok(frame) => {
                        if let Some(control) = frame_control(&frame) {
                            let cf = ControlField::parse(control);
                            if !cf.prm && cf.function == 11 {
                                link_state = LinkState::Busy;
                            }
                        }
                        let entry = frame_log_entry("rx", &frame, &link_config, &station_config);
                        push_log(&app, &shared, entry);
                        station.handle_frame(&frame);
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
            request = requests.recv() => {
                match request {
                    Some(MasterRequest::Interrogate { qoi }) => {
                        if !station.interrogate(qoi) {
                            push_log(&app, &shared, LogEntry::new("error", "总召唤已在进行中"));
                        }
                    }
                    Some(MasterRequest::Control { ioa, on }) => {
                        if !station.control(ioa, on) {
                            push_log(
                                &app,
                                &shared,
                                LogEntry::new("error", format!("遥控命令未接受: IOA={ioa}")),
                            );
                        }
                    }
                    Some(MasterRequest::SyncClock { time }) => {
                        if !station.sync_clock(time) {
                            push_log(
                                &app,
                                &shared,
                                LogEntry::new("error", "时钟同步命令未接受"),
                            );
                        }
                    }
                    None => break,
                }
            }
            _ = timer.tick() => {
                // 先推进超时计时,到点时重发上一帧;否则按轮询节奏发下一帧。
                let frame = match station.tick(interval_ms) {
                    Some(frame) => Some(frame),
                    None => station.next_frame(),
                };
                if let Some(frame) = frame {
                    let entry = frame_log_entry("tx", &frame, &link_config, &station_config);
                    push_log(&app, &shared, entry);
                    if let Err(err) = connection.write_frame(&frame, &link_config).await {
                        push_log(
                            &app,
                            &shared,
                            LogEntry::new("error", format!("写帧失败: {err}")),
                        );
                        break;
                    }
                }
            }
        }

        for event in station.drain_events() {
            match event {
                MasterEvent101::LinkOk => {
                    link_state = LinkState::Available;
                    push_log(&app, &shared, LogEntry::new("info", "链路可用"));
                }
                MasterEvent101::InterrogationStarted => {
                    push_log(&app, &shared, LogEntry::new("info", "总召唤开始"));
                }
                MasterEvent101::InterrogationFinished => {
                    push_log(&app, &shared, LogEntry::new("info", "总召唤结束"));
                }
                MasterEvent101::PointUpdate { ioa, value } => {
                    let json = point_value_to_json(&value);
                    if let Ok(mut guard) = shared.lock() {
                        if let Some(point) = guard.points.iter_mut().find(|p| p.ioa == ioa) {
                            point.value = json.clone();
                        } else {
                            let kind = point_kind_from_value(&value).to_string();
                            guard.points.push(PointView {
                                ioa,
                                kind,
                                value: json.clone(),
                                quality: String::new(),
                            });
                        }
                    }
                    push_log(
                        &app,
                        &shared,
                        LogEntry::new("info", format!("更新 IOA={ioa} 值={json}")),
                    );
                }
                MasterEvent101::CommandFailed { reason } => {
                    push_log(
                        &app,
                        &shared,
                        LogEntry::new("error", format!("命令失败: {reason}")),
                    );
                }
                MasterEvent101::ControlConfirmed { ioa, on } => {
                    push_log(
                        &app,
                        &shared,
                        LogEntry::new(
                            "info",
                            format!("遥控确认 IOA={ioa} {}", if on { "合" } else { "分" }),
                        ),
                    );
                }
                MasterEvent101::ClockSynced => {
                    push_log(&app, &shared, LogEntry::new("info", "时钟同步完成"));
                }
            }
        }

        let stats = station.stats();
        let snapshot = if let Ok(mut guard) = shared.lock() {
            guard.link_state = if guard.connected {
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

    if let Ok(mut guard) = shared.lock() {
        guard.connected = false;
        guard.link_state = "idle".to_string();
        guard.request_tx = None;
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async fn open_connection(transport: &TransportConfig) -> Result<FrameConnection, String> {
    match transport.mode.as_str() {
        "tcp" => {
            let address = format!("{}:{}", transport.host, transport.port);
            FrameConnection::connect_tcp(&address)
                .await
                .map_err(|err| format!("TCP 连接失败: {err}"))
        }
        "serial" => {
            let settings = SerialSettings {
                port: transport.serial_port.clone(),
                baud_rate: transport.baud_rate,
                data_bits: transport.data_bits,
                parity: parse_parity(&transport.parity)?,
                stop_bits: transport.stop_bits,
            };
            FrameConnection::open_serial(&settings)
                .await
                .map_err(|err| format!("打开串口失败: {err}"))
        }
        other => Err(format!("不支持的传输模式: {other}")),
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

fn point_kind_from_value(value: &PointValue) -> &'static str {
    match value {
        PointValue::Bool(_) => "single",
        PointValue::Double(_) => "double",
        PointValue::Normalized(_) => "normalized",
        PointValue::Scaled(_) => "scaled",
        PointValue::Float(_) => "float",
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
