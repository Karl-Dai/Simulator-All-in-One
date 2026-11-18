use crate::state::{
    AppState, CounterView, LogEntry, MasterSnapshot, SharedState, StatsView, MAX_LOGS,
};
use iec102_core::asdu::{decode_asdu102, Qcc, StationAddress};
use iec102_core::ft12::{encode_frame, ControlField, Frame, LinkConfig};
use iec102_core::link::{LinkParams, LinkState};
use iec102_core::session::{
    MasterEvent102, MasterRequest102, MasterStation102, StationConfig102,
};
use iec102_core::transport::{FrameConnection, Parity, SerialSettings};
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
pub struct MasterConnectConfig {
    pub transport: TransportConfig,
    pub link: LinkConfigDto,
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
    let station = MasterStation102::new(station_config);
    let (tx, rx) = mpsc::channel::<MasterRequest102>(64);

    if let Ok(mut guard) = state.shared.lock() {
        guard.connected = true;
        guard.link_state = "reset".to_string();
        guard.counters.clear();
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
pub async fn interrogate(
    state: State<'_, AppState>,
    request: u8,
    freeze: u8,
) -> Result<(), String> {
    if request > 4 {
        return Err("请求分组必须是 0 到 4".to_string());
    }
    if freeze > 3 {
        return Err("冻结方式必须是 0 到 3".to_string());
    }
    let tx = if let Ok(guard) = state.shared.lock() {
        guard.request_tx.clone()
    } else {
        None
    };
    match tx {
        Some(tx) => tx
            .send(MasterRequest102::Interrogate {
                qcc: Qcc { request, freeze },
            })
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
    mut connection: FrameConnection,
    mut station: MasterStation102,
    station_config: StationConfig102,
    mut requests: mpsc::Receiver<MasterRequest102>,
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
                    Some(MasterRequest102::Interrogate { qcc }) => {
                        if !station.interrogate(qcc) {
                            push_log(&app, &shared, LogEntry::new("error", "计量召唤已在进行中"));
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
                MasterEvent102::LinkOk => {
                    link_state = LinkState::Available;
                    push_log(&app, &shared, LogEntry::new("info", "链路可用"));
                }
                MasterEvent102::InterrogationStarted => {
                    push_log(&app, &shared, LogEntry::new("info", "计量召唤开始"));
                }
                MasterEvent102::InterrogationFinished => {
                    push_log(&app, &shared, LogEntry::new("info", "计量召唤结束"));
                }
                MasterEvent102::CounterUpdate {
                    ioa,
                    reading,
                    sequence,
                } => {
                    if let Ok(mut guard) = shared.lock() {
                        if let Some(counter) = guard.counters.iter_mut().find(|c| c.ioa == ioa) {
                            counter.reading = reading;
                            counter.sequence = sequence;
                        } else {
                            guard.counters.push(CounterView {
                                ioa,
                                reading,
                                sequence,
                            });
                        }
                    }
                    push_log(
                        &app,
                        &shared,
                        LogEntry::new(
                            "info",
                            format!("更新 IOA={ioa} 读数={reading} 顺序号={sequence}"),
                        ),
                    );
                }
                MasterEvent102::CommandFailed { reason } => {
                    push_log(
                        &app,
                        &shared,
                        LogEntry::new("error", format!("命令失败: {reason}")),
                    );
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
