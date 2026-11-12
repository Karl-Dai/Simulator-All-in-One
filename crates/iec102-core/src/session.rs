//! 102 主站与子站的业务会话。
//!
//! 会话层在链路层之上，负责处理累计量 ASDU 业务：
//!
//! - 子站响应累计量召唤，把激活确认、计数帧与激活终止放进二级队列。
//! - 主站发起累计量召唤，收集计数点更新并跟踪召唤进度。
//!
//! 链路帧的应答由 `link::SlaveLink` 与 `link::MasterLink` 负责，会话只处理
//! ASDU 与队列。

use crate::asdu::{
    decode_asdu102, encode_asdu102, Asdu102, BinaryCounter, InformationObject102, Qcc,
    StationAddress,
};
use crate::ft12::{ControlField, Frame, LinkConfig};
use crate::link::{LinkParams, MasterEvent, MasterLink, SlaveLink};
use crate::transport::FrameConnection;

/// 每帧最多上送的计量点数量。
const MAX_POINTS_PER_FRAME: usize = 20;

/// 累计量信息 M_IT_NA_1。
const TYPE_COUNTER: u8 = 100;
/// 累计量召唤 C_IT_NA_1。
const TYPE_COUNTER_INTERROGATION: u8 = 120;

/// 传送原因：激活。
const CAUSE_ACTIVATION: u8 = 6;
/// 传送原因：激活确认。
const CAUSE_ACTIVATION_CONFIRM: u8 = 7;
/// 传送原因：激活终止。
const CAUSE_ACTIVATION_TERMINATED: u8 = 10;
/// 传送原因：请求计数量。
const CAUSE_COUNTER_RESPONSE: u8 = 37;
/// 传送原因：否定确认。
const CAUSE_NEGATIVE_CONFIRM: u8 = 45;

/// 主站功能码：复位链路。
const FC_RESET_LINK: u8 = 0;
/// 主站功能码：发送/确认用户数据。
const FC_SEND_CONFIRM: u8 = 2;
/// 主站功能码：发送/无回答用户数据。
const FC_SEND_NO_REPLY: u8 = 4;

/// 站点配置。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StationConfig102 {
    /// 链路参数。
    pub link: LinkParams,
    /// ASDU 地址配置。
    pub address: StationAddress,
    /// 轮询间隔，毫秒。
    pub poll_interval_ms: u64,
    /// 响应超时，毫秒。
    pub response_timeout_ms: u64,
}

/// 计量点。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CounterPoint {
    /// 信息对象地址。
    pub ioa: u32,
    /// 累计量读数。
    pub reading: u32,
}

/// 会话统计。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SessionStats102 {
    /// 收到的帧数。
    pub frames_rx: u64,
    /// 发出的帧数。
    pub frames_tx: u64,
    /// 收到的 ASDU 数。
    pub asdu_rx: u64,
    /// 发出的 ASDU 数。
    pub asdu_tx: u64,
    /// 累计量召唤次数。
    pub interrogations: u64,
    /// 冻结次数。
    pub freezes: u64,
    /// 错误次数。
    pub errors: u64,
}

/// 子站事件。
#[derive(Debug, Clone, PartialEq)]
pub enum StationEvent102 {
    /// 链路复位。
    LinkReset,
    /// 累计量召唤开始，携带 QCC。
    InterrogationStarted {
        /// 累计量召唤限定词。
        qcc: Qcc,
    },
    /// 累计量召唤结束。
    InterrogationFinished,
    /// 计量点被冻结。
    CounterFrozen {
        /// 信息对象地址。
        ioa: u32,
        /// 冻结时的读数。
        reading: u32,
    },
    /// 计量点被复位。
    CounterReset {
        /// 信息对象地址。
        ioa: u32,
    },
}

/// 主站事件。
#[derive(Debug, Clone, PartialEq)]
pub enum MasterEvent102 {
    /// 链路可用。
    LinkOk,
    /// 累计量召唤开始。
    InterrogationStarted,
    /// 计量点更新。
    CounterUpdate {
        /// 信息对象地址。
        ioa: u32,
        /// 累计量读数。
        reading: u32,
        /// 顺序号。
        sequence: u8,
    },
    /// 累计量召唤结束。
    InterrogationFinished,
    /// 命令失败。
    CommandFailed {
        /// 失败原因。
        reason: String,
    },
}

/// 主站请求。
#[derive(Debug, Clone, PartialEq)]
pub enum MasterRequest102 {
    /// 累计量召唤。
    Interrogate {
        /// 累计量召唤限定词。
        qcc: Qcc,
    },
}

/// 一个计量点的内部状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CounterPointState {
    ioa: u32,
    reading: u32,
    /// 顺序号，取值 0..=31。
    sequence: u8,
}

/// 从链路帧提取 ASDU 字节。
///
/// 只处理主站方向（PRM=1）的发送/确认用户数据（功能码 2）与发送/无回答
/// （功能码 4）。其他帧不携带 ASDU。
fn extract_asdu(frame: &Frame) -> Option<Vec<u8>> {
    match frame {
        Frame::Variable { control, asdu, .. } => {
            let cf = ControlField::parse(*control);
            if cf.prm && (cf.function == FC_SEND_CONFIRM || cf.function == FC_SEND_NO_REPLY) {
                Some(asdu.clone())
            } else {
                None
            }
        }
        _ => None,
    }
}

/// 判断是否为复位链路帧。
fn is_reset_link(frame: &Frame) -> bool {
    match frame {
        Frame::Fixed { control, .. } => {
            let cf = ControlField::parse(*control);
            cf.prm && cf.function == FC_RESET_LINK
        }
        _ => false,
    }
}

/// 子站会话。
pub struct SlaveStation102 {
    config: StationConfig102,
    link: SlaveLink,
    points: Vec<CounterPointState>,
    events: Vec<StationEvent102>,
    stats: SessionStats102,
}

impl SlaveStation102 {
    /// 创建子站会话，初始顺序号都是 0。
    pub fn new(config: StationConfig102, points: Vec<CounterPoint>) -> Self {
        let points = points
            .into_iter()
            .map(|point| CounterPointState {
                ioa: point.ioa,
                reading: point.reading,
                sequence: 0,
            })
            .collect();
        Self {
            config,
            link: SlaveLink::new(config.link),
            points,
            events: Vec::new(),
            stats: SessionStats102::default(),
        }
    }

    /// 处理一帧，返回需要回给主站的帧；不需要应答时返回 `None`。
    pub fn handle_frame(&mut self, frame: &Frame) -> Option<Frame> {
        self.stats.frames_rx += 1;
        if is_reset_link(frame) {
            self.events.push(StationEvent102::LinkReset);
        }
        if let Some(asdu) = extract_asdu(frame) {
            self.stats.asdu_rx += 1;
            self.handle_asdu(&asdu);
        }
        let reply = self.link.handle(frame);
        if reply.is_some() {
            self.stats.frames_tx += 1;
        }
        reply
    }

    /// 处理收到的 ASDU。
    fn handle_asdu(&mut self, bytes: &[u8]) {
        match decode_asdu102(bytes, self.config.address) {
            Ok((asdu, _)) => {
                if asdu.type_id == TYPE_COUNTER_INTERROGATION && asdu.cause == CAUSE_ACTIVATION {
                    self.start_interrogation(&asdu);
                } else {
                    self.stats.errors += 1;
                    self.queue_negative_confirm(&asdu);
                }
            }
            Err(_) => {
                self.stats.errors += 1;
            }
        }
    }

    /// 响应累计量召唤：排队激活确认、计数帧与激活终止。
    fn start_interrogation(&mut self, asdu: &Asdu102) {
        let (ioa, qcc) = match asdu.objects.first() {
            Some(InformationObject102::CounterInterrogation { ioa, qcc }) => (*ioa, *qcc),
            _ => (
                0,
                Qcc {
                    request: 0,
                    freeze: 0,
                },
            ),
        };
        self.stats.interrogations += 1;
        if qcc.freeze == 1 || qcc.freeze == 2 {
            self.stats.freezes += 1;
        }
        self.events
            .push(StationEvent102::InterrogationStarted { qcc });

        // 激活确认。
        let confirm = self.make_asdu(
            TYPE_COUNTER_INTERROGATION,
            CAUSE_ACTIVATION_CONFIRM,
            vec![InformationObject102::CounterInterrogation { ioa, qcc }],
        );
        self.push_secondary(&confirm);

        // 冻结方式 0 直接读当前值，先取快照。
        let mut snapshot: Vec<(u32, u32, u8)> = self
            .points
            .iter()
            .map(|point| (point.ioa, point.reading, point.sequence))
            .collect();
        match qcc.freeze {
            1 => {
                for point in &self.points {
                    self.events.push(StationEvent102::CounterFrozen {
                        ioa: point.ioa,
                        reading: point.reading,
                    });
                }
            }
            2 => {
                // 先读，后清零。
                for point in &self.points {
                    self.events.push(StationEvent102::CounterFrozen {
                        ioa: point.ioa,
                        reading: point.reading,
                    });
                }
                for point in self.points.iter_mut() {
                    point.reading = 0;
                    self.events
                        .push(StationEvent102::CounterReset { ioa: point.ioa });
                }
            }
            3 => {
                // 先清零，后读。
                for point in self.points.iter_mut() {
                    point.reading = 0;
                    self.events
                        .push(StationEvent102::CounterReset { ioa: point.ioa });
                }
                snapshot = self
                    .points
                    .iter()
                    .map(|point| (point.ioa, point.reading, point.sequence))
                    .collect();
            }
            _ => {}
        }

        // 计数帧，每帧最多 20 个。
        for chunk in snapshot.chunks(MAX_POINTS_PER_FRAME) {
            let objects = chunk
                .iter()
                .map(|(ioa, reading, sequence)| InformationObject102::Counter {
                    ioa: *ioa,
                    counter: BinaryCounter {
                        reading: *reading,
                        sequence: *sequence,
                        carry: false,
                        adjusted: false,
                        invalid: false,
                    },
                })
                .collect();
            let frame = self.make_asdu(TYPE_COUNTER, CAUSE_COUNTER_RESPONSE, objects);
            self.push_secondary(&frame);
        }

        // 激活终止。
        let terminated = self.make_asdu(
            TYPE_COUNTER_INTERROGATION,
            CAUSE_ACTIVATION_TERMINATED,
            vec![InformationObject102::CounterInterrogation { ioa, qcc }],
        );
        self.push_secondary(&terminated);

        self.events.push(StationEvent102::InterrogationFinished);
    }

    /// 构造一个 ASDU，地址与本端配置一致。
    fn make_asdu(&self, type_id: u8, cause: u8, objects: Vec<InformationObject102>) -> Asdu102 {
        Asdu102 {
            type_id,
            sq: false,
            cause,
            negative: false,
            test: false,
            originator: 0,
            address: self.config.address,
            objects,
        }
    }

    /// 编码并排队一个 ASDU 到二级队列。
    fn push_secondary(&mut self, asdu: &Asdu102) {
        if let Ok(bytes) = encode_asdu102(asdu) {
            self.link.push_secondary(bytes);
            self.stats.asdu_tx += 1;
        }
    }

    /// 排队否定确认，类型标识与信息对象沿用收到的 ASDU。
    fn queue_negative_confirm(&mut self, asdu: &Asdu102) {
        let mut negative =
            self.make_asdu(asdu.type_id, CAUSE_NEGATIVE_CONFIRM, asdu.objects.clone());
        negative.negative = true;
        self.push_secondary(&negative);
    }

    /// 修改读数；读数变化时顺序号加 1，在 0..=31 循环。
    ///
    /// 点位不存在时返回 false。
    pub fn set_reading(&mut self, ioa: u32, reading: u32) -> bool {
        let point = match self.points.iter_mut().find(|point| point.ioa == ioa) {
            Some(point) => point,
            None => return false,
        };
        if point.reading != reading {
            point.reading = reading;
            point.sequence = (point.sequence + 1) & 0x1F;
        }
        true
    }

    /// 返回计量点当前读数。
    pub fn reading(&self, ioa: u32) -> Option<u32> {
        self.points
            .iter()
            .find(|point| point.ioa == ioa)
            .map(|point| point.reading)
    }

    /// 返回计量点当前顺序号。
    pub fn sequence(&self, ioa: u32) -> Option<u8> {
        self.points
            .iter()
            .find(|point| point.ioa == ioa)
            .map(|point| point.sequence)
    }

    /// 取出并清空待上报事件。
    pub fn drain_events(&mut self) -> Vec<StationEvent102> {
        std::mem::take(&mut self.events)
    }

    /// 返回会话统计。
    pub fn stats(&self) -> SessionStats102 {
        self.stats
    }
}

/// 主站会话。
pub struct MasterStation102 {
    config: StationConfig102,
    link: MasterLink,
    events: Vec<MasterEvent102>,
    received: Vec<(u32, u32, u8)>,
    stats: SessionStats102,
    /// 是否正在召唤中。
    interrogating: bool,
}

impl MasterStation102 {
    /// 创建主站会话。
    pub fn new(config: StationConfig102) -> Self {
        Self {
            config,
            link: MasterLink::new(config.link),
            events: Vec::new(),
            received: Vec::new(),
            stats: SessionStats102::default(),
            interrogating: false,
        }
    }

    /// 处理子站应答帧。
    pub fn handle_frame(&mut self, frame: &Frame) {
        self.stats.frames_rx += 1;
        match self.link.handle(frame) {
            MasterEvent::LinkOk => {
                self.events.push(MasterEvent102::LinkOk);
            }
            MasterEvent::Data { asdu, .. } => {
                self.handle_asdu(&asdu);
            }
            MasterEvent::Unexpected { reason } => {
                self.stats.errors += 1;
                self.events.push(MasterEvent102::CommandFailed { reason });
            }
            _ => {}
        }
    }

    /// 处理收到的 ASDU。
    fn handle_asdu(&mut self, bytes: &[u8]) {
        self.stats.asdu_rx += 1;
        let asdu = match decode_asdu102(bytes, self.config.address) {
            Ok((asdu, _)) => asdu,
            Err(err) => {
                self.stats.errors += 1;
                self.events.push(MasterEvent102::CommandFailed {
                    reason: format!("ASDU 解码失败: {:?}", err),
                });
                return;
            }
        };
        if asdu.negative {
            self.stats.errors += 1;
            self.events.push(MasterEvent102::CommandFailed {
                reason: format!("收到否定确认: 类型 {}, 原因 {}", asdu.type_id, asdu.cause),
            });
            return;
        }
        match (asdu.type_id, asdu.cause) {
            (TYPE_COUNTER_INTERROGATION, CAUSE_ACTIVATION_CONFIRM) => {
                self.events.push(MasterEvent102::InterrogationStarted);
            }
            (TYPE_COUNTER_INTERROGATION, CAUSE_ACTIVATION_TERMINATED) => {
                self.events.push(MasterEvent102::InterrogationFinished);
                self.interrogating = false;
            }
            (TYPE_COUNTER, CAUSE_COUNTER_RESPONSE) => {
                for object in &asdu.objects {
                    if let InformationObject102::Counter { ioa, counter } = object {
                        self.received
                            .push((*ioa, counter.reading, counter.sequence));
                        self.events.push(MasterEvent102::CounterUpdate {
                            ioa: *ioa,
                            reading: counter.reading,
                            sequence: counter.sequence,
                        });
                    }
                }
            }
            _ => {}
        }
    }

    /// 生成下一帧。
    pub fn next_frame(&mut self) -> Option<Frame> {
        let frame = self.link.poll();
        self.stats.frames_tx += 1;
        Some(frame)
    }

    /// 发起累计量召唤。
    ///
    /// 把累计量召唤 ASDU 排入发送队列。已在召唤中时返回 false。
    pub fn interrogate(&mut self, qcc: Qcc) -> bool {
        if self.interrogating {
            return false;
        }
        let asdu = Asdu102 {
            type_id: TYPE_COUNTER_INTERROGATION,
            sq: false,
            cause: CAUSE_ACTIVATION,
            negative: false,
            test: false,
            originator: 0,
            address: self.config.address,
            objects: vec![InformationObject102::CounterInterrogation { ioa: 0, qcc }],
        };
        match encode_asdu102(&asdu) {
            Ok(bytes) => {
                self.link.send_user_data(bytes);
                self.interrogating = true;
                self.stats.interrogations += 1;
                self.stats.asdu_tx += 1;
                true
            }
            Err(_) => false,
        }
    }

    /// 取出并清空待上报事件。
    pub fn drain_events(&mut self) -> Vec<MasterEvent102> {
        std::mem::take(&mut self.events)
    }

    /// 返回已收到的计量点（信息对象地址、读数、顺序号）。
    pub fn received(&self) -> Vec<(u32, u32, u8)> {
        self.received.clone()
    }

    /// 返回会话统计。
    pub fn stats(&self) -> SessionStats102 {
        self.stats
    }
}

/// 把子站事件转发到通道，通道满或关闭时忽略。
fn forward_slave_events(
    station: &mut SlaveStation102,
    events: &tokio::sync::mpsc::Sender<StationEvent102>,
) {
    for event in station.drain_events() {
        let _ = events.try_send(event);
    }
}

/// 把主站事件转发到通道，通道满或关闭时忽略。
fn forward_master_events(
    station: &mut MasterStation102,
    events: &tokio::sync::mpsc::Sender<MasterEvent102>,
) {
    for event in station.drain_events() {
        let _ = events.try_send(event);
    }
}

/// 读一帧，对端关闭连接时返回 `None`。
async fn read_frame_or_eof(
    connection: &mut FrameConnection,
    config: &LinkConfig,
) -> std::io::Result<Option<Frame>> {
    match connection.read_frame(config).await {
        Ok(frame) => Ok(Some(frame)),
        Err(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => Ok(None),
        Err(err) => Err(err),
    }
}

/// 运行子站会话，直到连接关闭。
///
/// 同时等待读帧与 50 毫秒定时器。读到帧就调用 `handle_frame` 并把应答帧写回。
/// 事件用 `try_send` 转发，通道满或关闭时忽略。
pub async fn run_slave102(
    mut connection: FrameConnection,
    mut station: SlaveStation102,
    events: tokio::sync::mpsc::Sender<StationEvent102>,
) -> std::io::Result<SlaveStation102> {
    let link_config = LinkConfig::new(station.config.link.address_len);
    let mut timer = tokio::time::interval(std::time::Duration::from_millis(50));
    loop {
        tokio::select! {
            result = read_frame_or_eof(&mut connection, &link_config) => {
                let frame = match result? {
                    Some(frame) => frame,
                    None => return Ok(station),
                };
                if let Some(reply) = station.handle_frame(&frame) {
                    connection.write_frame(&reply, &link_config).await?;
                }
                forward_slave_events(&mut station, &events);
            }
            _ = timer.tick() => {
                forward_slave_events(&mut station, &events);
            }
        }
    }
}

/// 运行主站会话，直到连接关闭或请求通道关闭。
///
/// 按 `config.poll_interval_ms` 间隔调用 `next_frame` 写帧，同时等待读帧与
/// 请求通道。收到帧调用 `handle_frame` 后把新事件转发出去。
pub async fn run_master102(
    mut connection: FrameConnection,
    mut station: MasterStation102,
    mut requests: tokio::sync::mpsc::Receiver<MasterRequest102>,
    events: tokio::sync::mpsc::Sender<MasterEvent102>,
) -> std::io::Result<MasterStation102> {
    let link_config = LinkConfig::new(station.config.link.address_len);
    let interval_ms = station.config.poll_interval_ms.max(1);
    let mut timer = tokio::time::interval(std::time::Duration::from_millis(interval_ms));
    loop {
        tokio::select! {
            result = read_frame_or_eof(&mut connection, &link_config) => {
                let frame = match result? {
                    Some(frame) => frame,
                    None => return Ok(station),
                };
                station.handle_frame(&frame);
                forward_master_events(&mut station, &events);
            }
            request = requests.recv() => {
                match request {
                    Some(MasterRequest102::Interrogate { qcc }) => {
                        station.interrogate(qcc);
                    }
                    None => return Ok(station),
                }
                forward_master_events(&mut station, &events);
            }
            _ = timer.tick() => {
                if let Some(frame) = station.next_frame() {
                    connection.write_frame(&frame, &link_config).await?;
                }
                forward_master_events(&mut station, &events);
            }
        }
    }
}
