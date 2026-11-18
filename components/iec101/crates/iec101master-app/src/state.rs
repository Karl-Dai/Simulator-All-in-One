use iec101_core::session::MasterRequest;
use serde::Serialize;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

/// 日志环形缓冲的最大条数。
pub const MAX_LOGS: usize = 2000;

/// 一条通信日志。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub at: String,
    pub direction: String,
    pub hex: String,
    pub summary: String,
}

impl LogEntry {
    pub fn new(direction: &str, summary: impl Into<String>) -> Self {
        Self {
            at: now_string(),
            direction: direction.to_string(),
            hex: String::new(),
            summary: summary.into(),
        }
    }

    pub fn with_hex(direction: &str, summary: impl Into<String>, hex: String) -> Self {
        Self {
            at: now_string(),
            direction: direction.to_string(),
            hex,
            summary: summary.into(),
        }
    }
}

fn now_string() -> String {
    chrono::Local::now()
        .format("%Y-%m-%d %H:%M:%S%.3f")
        .to_string()
}

/// 点表视图中的一个点位。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PointView {
    pub ioa: u32,
    pub kind: String,
    pub value: serde_json::Value,
    pub quality: String,
}

/// 统计视图。
#[derive(Debug, Clone, Copy, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsView {
    pub frames_rx: u64,
    pub frames_tx: u64,
    pub errors: u64,
}

/// 推给前端的完整快照。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MasterSnapshot {
    pub connected: bool,
    pub link_state: String,
    pub points: Vec<PointView>,
    pub stats: StatsView,
    pub logs: Vec<LogEntry>,
}

/// 连接循环与命令共享的可变状态。
pub struct SharedState {
    pub connected: bool,
    pub link_state: String,
    pub points: Vec<PointView>,
    pub stats: StatsView,
    pub logs: VecDeque<LogEntry>,
    pub request_tx: Option<mpsc::Sender<MasterRequest>>,
}

impl Default for SharedState {
    fn default() -> Self {
        Self {
            connected: false,
            link_state: "idle".to_string(),
            points: Vec::new(),
            stats: StatsView::default(),
            logs: VecDeque::new(),
            request_tx: None,
        }
    }
}

impl SharedState {
    pub fn snapshot(&self) -> MasterSnapshot {
        MasterSnapshot {
            connected: self.connected,
            link_state: self.link_state.clone(),
            points: self.points.clone(),
            stats: self.stats,
            logs: self.logs.iter().cloned().collect(),
        }
    }
}

/// Tauri 管理的应用状态。
pub struct AppState {
    /// 套件配置存储：界面偏好与工作区参数写入 SQLite。
    pub suite_storage: crate::suite_storage::SuiteStorage,
    /// tokio 运行时句柄，用于启动连接循环任务。
    pub runtime: tokio::runtime::Handle,
    /// 当前连接循环任务。
    pub task: Mutex<Option<tokio::task::JoinHandle<()>>>,
    /// 连接循环与命令共享的状态。
    pub shared: Arc<Mutex<SharedState>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            suite_storage: Default::default(),
            runtime: tauri::async_runtime::handle().inner().clone(),
            task: Mutex::new(None),
            shared: Arc::new(Mutex::new(SharedState::default())),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
