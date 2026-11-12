//! 主站与子站的业务会话。
//!
//! 会话层在链路层之上，负责处理 ASDU 业务逻辑：
//!
//! - 子站响应总召唤，把激活确认、点位数据与激活终止放进二级队列。
//! - 子站把 `set_value` 改动的点位作为突发数据放进一级队列。
//! - 主站发起总召唤，收集点位更新并跟踪召唤进度。
//!
//! 链路帧的应答由 `link::SlaveLink` 与 `link::MasterLink` 负责，会话只处理
//! ASDU 与队列。

use crate::asdu::{decode_asdu, encode_asdu, Asdu, AsduAddress, InformationObject, Quality};
use crate::ft12::{ControlField, Frame};
use crate::link::{LinkParams, MasterEvent, MasterLink, SlaveLink};

/// 每帧最多上送的点位数量。
const MAX_POINTS_PER_FRAME: usize = 20;

/// 单点信息 M_SP_NA_1。
const TYPE_SINGLE_POINT: u8 = 1;
/// 双点信息 M_DP_NA_1。
const TYPE_DOUBLE_POINT: u8 = 3;
/// 归一化测量值 M_ME_NA_1。
const TYPE_NORMALIZED: u8 = 9;
/// 标度化测量值 M_ME_NB_1。
const TYPE_SCALED: u8 = 11;
/// 短浮点测量值 M_ME_NC_1。
const TYPE_SHORT_FLOAT: u8 = 13;
/// 带时标单点信息 M_SP_TB_1。
const TYPE_SINGLE_POINT_TIME: u8 = 30;
/// 带时标双点信息 M_DP_TB_1。
const TYPE_DOUBLE_POINT_TIME: u8 = 31;
/// 带时标归一化测量值 M_ME_TD_1。
const TYPE_NORMALIZED_TIME: u8 = 34;
/// 带时标标度化测量值 M_ME_TE_1。
const TYPE_SCALED_TIME: u8 = 35;
/// 带时标短浮点测量值 M_ME_TF_1。
const TYPE_SHORT_FLOAT_TIME: u8 = 36;
/// 总召唤 C_IC_NA_1。
const TYPE_INTERROGATION: u8 = 100;

/// 传送原因：突发。
const CAUSE_SPONTANEOUS: u8 = 3;
/// 传送原因：激活。
const CAUSE_ACTIVATION: u8 = 6;
/// 传送原因：激活确认。
const CAUSE_ACTIVATION_CONFIRM: u8 = 7;
/// 传送原因：激活终止。
const CAUSE_ACTIVATION_TERMINATED: u8 = 10;
/// 传送原因：响应总召唤。
const CAUSE_INTERROGATION_RESPONSE: u8 = 20;
/// 传送原因：否定确认。
const CAUSE_NEGATIVE_CONFIRM: u8 = 45;

/// 主站功能码：复位链路。
const FC_RESET_LINK: u8 = 0;
/// 主站功能码：发送/确认用户数据。
const FC_SEND_CONFIRM: u8 = 2;
/// 主站功能码：发送/无回答用户数据。
const FC_SEND_NO_REPLY: u8 = 4;

/// 点位类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PointKind {
    /// 单点信息 M_SP_NA_1。
    SinglePoint,
    /// 双点信息 M_DP_NA_1。
    DoublePoint,
    /// 归一化测量值 M_ME_NA_1。
    Normalized,
    /// 标度化测量值 M_ME_NB_1。
    Scaled,
    /// 短浮点测量值 M_ME_NC_1。
    ShortFloat,
}

impl PointKind {
    /// 返回对应的 ASDU 类型标识。
    fn type_id(self) -> u8 {
        match self {
            Self::SinglePoint => TYPE_SINGLE_POINT,
            Self::DoublePoint => TYPE_DOUBLE_POINT,
            Self::Normalized => TYPE_NORMALIZED,
            Self::Scaled => TYPE_SCALED,
            Self::ShortFloat => TYPE_SHORT_FLOAT,
        }
    }

    /// 判断点位值是否与类型匹配。
    fn matches(self, value: &PointValue) -> bool {
        matches!(
            (self, value),
            (Self::SinglePoint, PointValue::Bool(_))
                | (Self::DoublePoint, PointValue::Double(_))
                | (Self::Normalized, PointValue::Normalized(_))
                | (Self::Scaled, PointValue::Scaled(_))
                | (Self::ShortFloat, PointValue::Float(_))
        )
    }

    /// 返回该类型的初始值。
    fn initial_value(self) -> PointValue {
        match self {
            Self::SinglePoint => PointValue::Bool(false),
            Self::DoublePoint => PointValue::Double(0),
            Self::Normalized => PointValue::Normalized(0),
            Self::Scaled => PointValue::Scaled(0),
            Self::ShortFloat => PointValue::Float(0.0),
        }
    }
}

/// 点位值。
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum PointValue {
    /// 单点值。
    Bool(bool),
    /// 双点值，取 0..=3。
    Double(u8),
    /// 归一化值。
    Normalized(i16),
    /// 标度化值。
    Scaled(i16),
    /// 短浮点值。
    Float(f32),
}

/// 站点配置。
///
/// `LinkParams` 与 `AsduAddress` 未派生 serde，这里不派生 Serialize/Deserialize。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StationConfig {
    /// 链路参数。
    pub link: LinkParams,
    /// ASDU 地址配置。
    pub address: AsduAddress,
    /// 轮询间隔，毫秒。
    pub poll_interval_ms: u64,
    /// 响应超时，毫秒。
    pub response_timeout_ms: u64,
}

/// 会话统计。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SessionStats {
    /// 收到的帧数。
    pub frames_rx: u64,
    /// 发出的帧数。
    pub frames_tx: u64,
    /// 收到的 ASDU 数。
    pub asdu_rx: u64,
    /// 发出的 ASDU 数。
    pub asdu_tx: u64,
    /// 总召唤次数。
    pub interrogations: u64,
    /// 错误次数。
    pub errors: u64,
}

/// 子站事件。
#[derive(Debug, Clone, PartialEq)]
pub enum StationEvent {
    /// 链路复位。
    LinkReset,
    /// 总召唤开始，携带 QOI。
    InterrogationStarted {
        /// 召唤限定词。
        qoi: u8,
    },
    /// 总召唤结束。
    InterrogationFinished,
    /// 点位值改变。
    PointChanged {
        /// 信息对象地址。
        ioa: u32,
    },
}

/// 主站事件。
#[derive(Debug, Clone, PartialEq)]
pub enum MasterEvent101 {
    /// 链路可用。
    LinkOk,
    /// 总召唤开始。
    InterrogationStarted,
    /// 点位更新。
    PointUpdate {
        /// 信息对象地址。
        ioa: u32,
        /// 点位值。
        value: PointValue,
    },
    /// 总召唤结束。
    InterrogationFinished,
    /// 命令失败。
    CommandFailed {
        /// 失败原因。
        reason: String,
    },
}

/// 一个点位。
struct Point {
    ioa: u32,
    kind: PointKind,
    value: PointValue,
}

impl Point {
    /// 转换成 ASDU 信息对象。
    fn to_information_object(&self) -> InformationObject {
        let quality = default_quality();
        match self.value {
            PointValue::Bool(value) => InformationObject::SinglePoint {
                ioa: self.ioa,
                value,
                quality,
            },
            PointValue::Double(value) => InformationObject::DoublePoint {
                ioa: self.ioa,
                value,
                quality,
            },
            PointValue::Normalized(value) => InformationObject::Normalized {
                ioa: self.ioa,
                value,
                quality,
            },
            PointValue::Scaled(value) => InformationObject::Scaled {
                ioa: self.ioa,
                value,
                quality,
            },
            PointValue::Float(value) => InformationObject::ShortFloat {
                ioa: self.ioa,
                value,
                quality,
            },
        }
    }
}

/// 默认品质位，所有标志为 false。
fn default_quality() -> Quality {
    Quality {
        blocked: false,
        substituted: false,
        not_topical: false,
        invalid: false,
    }
}

/// 从信息对象提取点位值，监视方向类型返回 Some。
fn object_to_point_value(object: &InformationObject) -> Option<(u32, PointValue)> {
    match object {
        InformationObject::SinglePoint { ioa, value, .. } => Some((*ioa, PointValue::Bool(*value))),
        InformationObject::DoublePoint { ioa, value, .. } => {
            Some((*ioa, PointValue::Double(*value)))
        }
        InformationObject::Normalized { ioa, value, .. } => {
            Some((*ioa, PointValue::Normalized(*value)))
        }
        InformationObject::Scaled { ioa, value, .. } => Some((*ioa, PointValue::Scaled(*value))),
        InformationObject::ShortFloat { ioa, value, .. } => {
            Some((*ioa, PointValue::Float(*value)))
        }
        InformationObject::SinglePointTime { ioa, value, .. } => {
            Some((*ioa, PointValue::Bool(*value)))
        }
        InformationObject::DoublePointTime { ioa, value, .. } => {
            Some((*ioa, PointValue::Double(*value)))
        }
        InformationObject::NormalizedTime { ioa, value, .. } => {
            Some((*ioa, PointValue::Normalized(*value)))
        }
        InformationObject::ScaledTime { ioa, value, .. } => {
            Some((*ioa, PointValue::Scaled(*value)))
        }
        InformationObject::ShortFloatTime { ioa, value, .. } => {
            Some((*ioa, PointValue::Float(*value)))
        }
        _ => None,
    }
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
pub struct SlaveStation {
    config: StationConfig,
    link: SlaveLink,
    points: Vec<Point>,
    events: Vec<StationEvent>,
    stats: SessionStats,
}

impl SlaveStation {
    /// 创建子站会话。
    ///
    /// `points` 为（信息对象地址，点位类型）列表，初始值由类型决定。
    pub fn new(config: StationConfig, points: Vec<(u32, PointKind)>) -> Self {
        let points = points
            .into_iter()
            .map(|(ioa, kind)| Point {
                ioa,
                kind,
                value: kind.initial_value(),
            })
            .collect();
        Self {
            config,
            link: SlaveLink::new(config.link),
            points,
            events: Vec::new(),
            stats: SessionStats::default(),
        }
    }

    /// 处理一帧，返回需要回给主站的帧；不需要应答时返回 `None`。
    pub fn handle_frame(&mut self, frame: &Frame) -> Option<Frame> {
        self.stats.frames_rx += 1;
        if is_reset_link(frame) {
            self.events.push(StationEvent::LinkReset);
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
        match decode_asdu(bytes, self.config.address) {
            Ok((asdu, _)) => {
                if asdu.type_id == TYPE_INTERROGATION && asdu.cause == CAUSE_ACTIVATION {
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

    /// 响应总召唤：排队激活确认、点位数据与激活终止。
    fn start_interrogation(&mut self, asdu: &Asdu) {
        let (ioa, qoi) = match asdu.objects.first() {
            Some(InformationObject::InterrogationCommand { ioa, qoi }) => (*ioa, *qoi),
            _ => (0, 0),
        };
        self.stats.interrogations += 1;
        self.events.push(StationEvent::InterrogationStarted { qoi });

        // 激活确认。
        let confirm = Asdu {
            type_id: TYPE_INTERROGATION,
            sq: false,
            cause: CAUSE_ACTIVATION_CONFIRM,
            negative: false,
            test: false,
            originator: 0,
            address: self.config.address,
            objects: vec![InformationObject::InterrogationCommand { ioa, qoi }],
        };
        self.push_secondary(&confirm);

        // 点位数据，按类型分组，每帧最多 20 个。
        for kind in [
            PointKind::SinglePoint,
            PointKind::DoublePoint,
            PointKind::Normalized,
            PointKind::Scaled,
            PointKind::ShortFloat,
        ] {
            let objects = self
                .points
                .iter()
                .filter(|point| point.kind == kind)
                .map(|point| point.to_information_object())
                .collect::<Vec<_>>();
            for chunk in objects.chunks(MAX_POINTS_PER_FRAME) {
                self.push_point_frame(kind, chunk);
            }
        }

        // 激活终止。
        let terminated = Asdu {
            type_id: TYPE_INTERROGATION,
            sq: false,
            cause: CAUSE_ACTIVATION_TERMINATED,
            negative: false,
            test: false,
            originator: 0,
            address: self.config.address,
            objects: vec![InformationObject::InterrogationCommand { ioa, qoi }],
        };
        self.push_secondary(&terminated);

        self.events.push(StationEvent::InterrogationFinished);
    }

    /// 编码并排队一帧点位数据。
    fn push_point_frame(&mut self, kind: PointKind, objects: &[InformationObject]) {
        let asdu = Asdu {
            type_id: kind.type_id(),
            sq: false,
            cause: CAUSE_INTERROGATION_RESPONSE,
            negative: false,
            test: false,
            originator: 0,
            address: self.config.address,
            objects: objects.to_vec(),
        };
        self.push_secondary(&asdu);
    }

    /// 编码并排队一个 ASDU 到二级队列。
    fn push_secondary(&mut self, asdu: &Asdu) {
        if let Ok(bytes) = encode_asdu(asdu) {
            self.link.push_secondary(bytes);
            self.stats.asdu_tx += 1;
        }
    }

    /// 排队否定确认，类型标识沿用收到的 ASDU。
    fn queue_negative_confirm(&mut self, asdu: &Asdu) {
        let negative = Asdu {
            type_id: asdu.type_id,
            sq: false,
            cause: CAUSE_NEGATIVE_CONFIRM,
            negative: true,
            test: false,
            originator: 0,
            address: self.config.address,
            objects: asdu.objects.clone(),
        };
        self.push_secondary(&negative);
    }

    /// 设置点位值。
    ///
    /// 点位不存在或值类型不匹配时返回 false。成功时把该点作为突发数据
    /// （原因 3）放入一级队列，并记录 [`StationEvent::PointChanged`]。
    pub fn set_value(&mut self, ioa: u32, value: PointValue) -> bool {
        let index = match self.points.iter().position(|p| p.ioa == ioa) {
            Some(index) => index,
            None => return false,
        };
        if !self.points[index].kind.matches(&value) {
            return false;
        }
        self.points[index].value = value;
        let asdu = Asdu {
            type_id: self.points[index].kind.type_id(),
            sq: false,
            cause: CAUSE_SPONTANEOUS,
            negative: false,
            test: false,
            originator: 0,
            address: self.config.address,
            objects: vec![self.points[index].to_information_object()],
        };
        if let Ok(bytes) = encode_asdu(&asdu) {
            self.link.push_primary(bytes);
            self.stats.asdu_tx += 1;
        }
        self.events.push(StationEvent::PointChanged { ioa });
        true
    }

    /// 返回点位当前值。
    pub fn value(&self, ioa: u32) -> Option<PointValue> {
        self.points.iter().find(|p| p.ioa == ioa).map(|p| p.value)
    }

    /// 取出并清空待上报事件。
    pub fn drain_events(&mut self) -> Vec<StationEvent> {
        std::mem::take(&mut self.events)
    }

    /// 返回会话统计。
    pub fn stats(&self) -> SessionStats {
        self.stats
    }
}

/// 主站会话。
pub struct MasterStation {
    config: StationConfig,
    link: MasterLink,
    events: Vec<MasterEvent101>,
    received: Vec<(u32, PointValue)>,
    stats: SessionStats,
    /// 是否正在召唤中。
    interrogating: bool,
}

impl MasterStation {
    /// 创建主站会话。
    pub fn new(config: StationConfig) -> Self {
        Self {
            config,
            link: MasterLink::new(config.link),
            events: Vec::new(),
            received: Vec::new(),
            stats: SessionStats::default(),
            interrogating: false,
        }
    }

    /// 处理子站应答帧。
    pub fn handle_frame(&mut self, frame: &Frame) {
        self.stats.frames_rx += 1;
        match self.link.handle(frame) {
            MasterEvent::LinkOk => {
                self.events.push(MasterEvent101::LinkOk);
            }
            MasterEvent::Data { asdu, .. } => {
                self.handle_asdu(&asdu);
            }
            MasterEvent::Unexpected { reason } => {
                self.stats.errors += 1;
                self.events.push(MasterEvent101::CommandFailed { reason });
            }
            _ => {}
        }
    }

    /// 处理收到的 ASDU。
    fn handle_asdu(&mut self, bytes: &[u8]) {
        self.stats.asdu_rx += 1;
        let asdu = match decode_asdu(bytes, self.config.address) {
            Ok((asdu, _)) => asdu,
            Err(err) => {
                self.stats.errors += 1;
                self.events.push(MasterEvent101::CommandFailed {
                    reason: format!("ASDU 解码失败: {:?}", err),
                });
                return;
            }
        };
        if asdu.negative || asdu.cause == CAUSE_NEGATIVE_CONFIRM {
            self.stats.errors += 1;
            self.events.push(MasterEvent101::CommandFailed {
                reason: format!(
                    "收到否定确认: 类型 {}, 原因 {}",
                    asdu.type_id, asdu.cause
                ),
            });
            return;
        }
        match asdu.type_id {
            TYPE_INTERROGATION if asdu.cause == CAUSE_ACTIVATION_CONFIRM => {
                self.events.push(MasterEvent101::InterrogationStarted);
            }
            TYPE_INTERROGATION if asdu.cause == CAUSE_ACTIVATION_TERMINATED => {
                self.events.push(MasterEvent101::InterrogationFinished);
                self.interrogating = false;
            }
            TYPE_SINGLE_POINT
            | TYPE_DOUBLE_POINT
            | TYPE_NORMALIZED
            | TYPE_SCALED
            | TYPE_SHORT_FLOAT
            | TYPE_SINGLE_POINT_TIME
            | TYPE_DOUBLE_POINT_TIME
            | TYPE_NORMALIZED_TIME
            | TYPE_SCALED_TIME
            | TYPE_SHORT_FLOAT_TIME => {
                for object in &asdu.objects {
                    if let Some((ioa, value)) = object_to_point_value(object) {
                        self.received.push((ioa, value));
                        self.events.push(MasterEvent101::PointUpdate { ioa, value });
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

    /// 发起总召唤。
    ///
    /// 把总召唤 ASDU 排入发送队列。已在召唤中时返回 false。
    pub fn interrogate(&mut self, qoi: u8) -> bool {
        if self.interrogating {
            return false;
        }
        let asdu = Asdu {
            type_id: TYPE_INTERROGATION,
            sq: false,
            cause: CAUSE_ACTIVATION,
            negative: false,
            test: false,
            originator: 0,
            address: self.config.address,
            objects: vec![InformationObject::InterrogationCommand { ioa: 0, qoi }],
        };
        match encode_asdu(&asdu) {
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
    pub fn drain_events(&mut self) -> Vec<MasterEvent101> {
        std::mem::take(&mut self.events)
    }

    /// 返回已收到的点位值。
    pub fn received(&self) -> Vec<(u32, PointValue)> {
        self.received.clone()
    }

    /// 返回会话统计。
    pub fn stats(&self) -> SessionStats {
        self.stats
    }
}
