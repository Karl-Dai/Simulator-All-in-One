//! 非平衡链路状态机。
//!
//! 非平衡链路中主站是启动站,负责复位链路、发送用户数据和轮询;子站只应答。
//! 主站复位链路后,第一帧用户数据的 FCB(帧计数位)固定为 1,之后每发一帧翻转
//! 一次(1, 0, 1, 0…)。子站用 `expected_fcb` 记录下一次期望收到的 FCB:收到的
//! FCB 与期望值相同就是新帧,接受后翻转期望值;不同就是重复帧,重发上一次的
//! 应答帧,不重复执行业务。
//!
//! 控制域功能码定义(低 4 位):
//!
//! - 主站(PRM=1):0 复位链路,1 复位 FCB,2 发送/确认用户数据,4 发送/无回答,
//!   9 请求链路状态,10 请求一级数据,11 请求二级数据。
//! - 子站(PRM=0):0 肯定认可,1 否定认可,8 用户数据,11 链路忙。
//!
//! 子站应答帧的 bit4 是 ACD(有一级数据待上送),bit5 是 DFC(链路忙)。

use std::collections::VecDeque;

use crate::ft12::{ControlField, Frame};

/// 链路参数。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinkParams {
    /// 链路地址。
    pub address: u16,
    /// 地址域长度,取 1 或 2。
    pub address_len: u8,
    /// 用户数据确认的最大重发次数。
    pub max_retries: u32,
}

/// 链路状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkState {
    /// 尚未完成复位。
    Reset,
    /// 链路可用。
    Available,
    /// 子站报告链路忙。
    Busy,
}

/// 主站功能码(PRM=1)。
const FC_RESET_LINK: u8 = 0;
const FC_RESET_FCB: u8 = 1;
const FC_SEND_CONFIRM: u8 = 2;
const FC_SEND_NO_REPLY: u8 = 4;
const FC_REQUEST_STATUS: u8 = 9;
const FC_REQUEST_PRIMARY: u8 = 10;
const FC_REQUEST_SECONDARY: u8 = 11;

/// 子站功能码(PRM=0)。
const FC_ACK: u8 = 0;
const FC_NAK: u8 = 1;
const FC_USER_DATA: u8 = 8;
const FC_LINK_BUSY: u8 = 11;

/// 构造主站方向(PRM=1)的控制域。
fn primary_control(function: u8, fcb: bool, fcv: bool) -> u8 {
    ControlField {
        dir: false,
        prm: true,
        fcb,
        fcv,
        acd: false,
        dfc: false,
        function,
    }
    .encode()
}

/// 构造子站方向(PRM=0)的控制域。
fn secondary_control(function: u8, acd: bool, dfc: bool) -> u8 {
    ControlField {
        dir: false,
        prm: false,
        fcb: false,
        fcv: false,
        acd,
        dfc,
        function,
    }
    .encode()
}

/// 子站链路。
///
/// 维护一级数据队列、二级数据队列和主站下发命令队列,根据主站请求产生应答帧。
pub struct SlaveLink {
    params: LinkParams,
    state: LinkState,
    /// 一级数据,需要尽快上送。
    primary: VecDeque<Vec<u8>>,
    /// 二级数据,普通周期数据。
    secondary: VecDeque<Vec<u8>>,
    /// 主站下发的用户数据,用于重复帧去重。
    commands: VecDeque<Vec<u8>>,
    /// 下一次期望收到的主站用户数据 FCB。复位链路后主站第一帧的 FCB 固定为 1,
    /// 所以初始值和复位后的值都是 `true`;每接受一个新帧就翻转一次。
    expected_fcb: bool,
    /// 上一次发出的应答帧,重复帧时重发。
    last_reply: Option<Frame>,
}

impl SlaveLink {
    /// 创建子站链路,初始状态为 [`LinkState::Reset`]。
    pub fn new(params: LinkParams) -> Self {
        Self {
            params,
            state: LinkState::Reset,
            primary: VecDeque::new(),
            secondary: VecDeque::new(),
            commands: VecDeque::new(),
            expected_fcb: true,
            last_reply: None,
        }
    }

    /// 返回当前链路状态。
    pub fn state(&self) -> LinkState {
        self.state
    }

    /// 加入一级数据(需要尽快上送),每条 ASDU 一帧。
    pub fn push_primary(&mut self, asdu: Vec<u8>) {
        self.primary.push_back(asdu);
    }

    /// 加入二级数据(普通周期数据)。
    pub fn push_secondary(&mut self, asdu: Vec<u8>) {
        self.secondary.push_back(asdu);
    }

    /// 返回是否有一级数据待上送。
    pub fn has_primary(&self) -> bool {
        !self.primary.is_empty()
    }

    /// 返回当前队列长度,顺序为(一级数据, 二级数据)。
    pub fn pending(&self) -> (usize, usize) {
        (self.primary.len(), self.secondary.len())
    }

    /// 返回已接受的主站下发命令数量,供测试与统计使用。
    pub fn command_count(&self) -> usize {
        self.commands.len()
    }

    /// 取出一条待处理的主站下发 ASDU。
    ///
    /// 只有链路层认定为新帧的 ASDU 才会入队,重复帧不会入队,因此会话层
    /// 取出处理即可保证业务只执行一次。
    pub fn take_command(&mut self) -> Option<Vec<u8>> {
        self.commands.pop_front()
    }

    /// 构造肯定认可帧。
    fn ack(&self, acd: bool) -> Frame {
        Frame::Fixed {
            control: secondary_control(FC_ACK, acd, false),
            address: self.params.address,
        }
    }

    /// 构造否定认可帧。
    fn nak(&self) -> Frame {
        Frame::Fixed {
            control: secondary_control(FC_NAK, false, false),
            address: self.params.address,
        }
    }

    /// 构造用户数据帧。
    fn data(&self, asdu: Vec<u8>, acd: bool) -> Frame {
        Frame::Variable {
            control: secondary_control(FC_USER_DATA, acd, false),
            address: self.params.address,
            asdu,
        }
    }

    /// 保存并返回应答帧。
    fn store(&mut self, frame: Frame) -> Option<Frame> {
        self.last_reply = Some(frame.clone());
        Some(frame)
    }

    /// 处理一帧,返回需要回给主站的帧;不需要应答时返回 `None`。
    pub fn handle(&mut self, frame: &Frame) -> Option<Frame> {
        let (control, address) = match frame {
            Frame::Fixed { control, address } => (*control, *address),
            Frame::Variable { control, address, .. } => (*control, *address),
            Frame::Ack => return None,
        };
        if address != self.params.address {
            return None;
        }
        let cf = ControlField::parse(control);
        if !cf.prm {
            return None;
        }
        match cf.function {
            FC_RESET_LINK => {
                self.primary.clear();
                self.secondary.clear();
                self.commands.clear();
                self.expected_fcb = true;
                self.last_reply = None;
                self.state = LinkState::Available;
                self.store(self.ack(false))
            }
            FC_RESET_FCB => {
                self.expected_fcb = true;
                self.store(self.ack(self.has_primary()))
            }
            FC_SEND_CONFIRM => {
                let asdu = match frame {
                    Frame::Variable { asdu, .. } => asdu.clone(),
                    Frame::Fixed { .. } => return self.store(self.nak()),
                    Frame::Ack => return None,
                };
                if cf.fcb == self.expected_fcb {
                    // FCB 与期望值相同,是新帧,接受并把期望值翻转到下一个。
                    self.expected_fcb = !cf.fcb;
                    self.commands.push_back(asdu);
                    self.store(self.ack(false))
                } else {
                    // FCB 与期望值不同,是重复帧,重发上一次应答,不入队、不翻转。
                    match self.last_reply.clone() {
                        Some(reply) => Some(reply),
                        None => self.store(self.ack(false)),
                    }
                }
            }
            FC_SEND_NO_REPLY => {
                if let Frame::Variable { asdu, .. } = frame {
                    self.commands.push_back(asdu.clone());
                }
                None
            }
            FC_REQUEST_STATUS => self.store(self.ack(self.has_primary())),
            FC_REQUEST_PRIMARY => {
                if let Some(asdu) = self.primary.pop_front() {
                    let acd = !self.primary.is_empty();
                    self.store(self.data(asdu, acd))
                } else {
                    self.store(self.ack(false))
                }
            }
            FC_REQUEST_SECONDARY => {
                // 一级数据优先。
                if let Some(asdu) = self.primary.pop_front() {
                    let acd = !self.primary.is_empty();
                    self.store(self.data(asdu, acd))
                } else if let Some(asdu) = self.secondary.pop_front() {
                    self.store(self.data(asdu, false))
                } else {
                    self.store(self.ack(false))
                }
            }
            _ => self.store(self.nak()),
        }
    }
}

/// 主站链路事件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MasterEvent {
    /// 链路复位完成,链路可用。
    LinkOk,
    /// 收到肯定认可。
    Ack {
        /// 子站是否有一级数据待上送。
        acd: bool,
        /// 子站是否报告链路忙。
        dfc: bool,
    },
    /// 收到用户数据。
    Data {
        /// ASDU 内容。
        asdu: Vec<u8>,
        /// 子站是否有一级数据待上送。
        acd: bool,
        /// 子站是否报告链路忙。
        dfc: bool,
    },
    /// 子站报告链路忙。
    LinkBusy,
    /// 链路复位。
    LinkReset,
    /// 超时。
    Timeout,
    /// 收到无法识别的帧。
    Unexpected {
        /// 原因描述。
        reason: String,
    },
}

/// 主站链路。
///
/// 负责生成复位帧、用户数据帧和轮询帧,并跟踪链路状态与等待确认的用户数据。
pub struct MasterLink {
    params: LinkParams,
    state: LinkState,
    /// 发送用户数据时使用的 FCB。复位链路后为 `true`,每发一帧用户数据翻转一次。
    fcb: bool,
    /// 待发送的用户数据。
    tx_queue: VecDeque<Vec<u8>>,
    /// 最近发出、等待确认的用户数据帧,超时时重发。
    last_sent: Option<Frame>,
    /// 是否正在等待用户数据确认。
    waiting_ack: bool,
    /// 已重发次数。
    retries: u32,
    /// 最近一次应答的 ACD,决定下一次轮询请求一级还是二级数据。
    last_acd: bool,
}

impl MasterLink {
    /// 创建主站链路,初始状态为 [`LinkState::Reset`]。
    pub fn new(params: LinkParams) -> Self {
        Self {
            params,
            state: LinkState::Reset,
            fcb: true,
            tx_queue: VecDeque::new(),
            last_sent: None,
            waiting_ack: false,
            retries: 0,
            last_acd: false,
        }
    }

    /// 返回当前链路状态。
    pub fn state(&self) -> LinkState {
        self.state
    }

    /// 返回是否正在等待用户数据确认。
    pub fn waiting_ack(&self) -> bool {
        self.waiting_ack
    }

    /// 生成复位链路帧,并把 FCB 重置为 `true`、等待状态复位。
    pub fn reset_link(&mut self) -> Frame {
        self.fcb = true;
        self.waiting_ack = false;
        self.last_sent = None;
        self.retries = 0;
        Frame::Fixed {
            control: primary_control(FC_RESET_LINK, false, false),
            address: self.params.address,
        }
    }

    /// 生成下一帧:有待发用户数据且已复位时发用户数据,否则请求一级或二级数据。
    ///
    /// `Reset` 状态返回复位链路帧;`Busy` 状态返回请求链路状态帧;等待用户数据
    /// 确认时重发上一帧。
    pub fn poll(&mut self) -> Frame {
        match self.state {
            LinkState::Reset => self.reset_link(),
            LinkState::Busy => self.request_status(),
            LinkState::Available => {
                if self.waiting_ack {
                    if let Some(last) = self.last_sent.clone() {
                        return last;
                    }
                }
                if let Some(asdu) = self.tx_queue.front().cloned() {
                    let control = primary_control(FC_SEND_CONFIRM, self.fcb, true);
                    let frame = Frame::Variable {
                        control,
                        address: self.params.address,
                        asdu,
                    };
                    self.fcb = !self.fcb;
                    self.last_sent = Some(frame.clone());
                    self.waiting_ack = true;
                    self.retries = 0;
                    self.tx_queue.pop_front();
                    frame
                } else if self.last_acd {
                    self.request_primary()
                } else {
                    self.request_secondary()
                }
            }
        }
    }

    /// 处理子站应答,更新链路状态并返回对应事件。
    pub fn handle(&mut self, frame: &Frame) -> MasterEvent {
        match frame {
            Frame::Ack => {
                self.waiting_ack = false;
                if self.state == LinkState::Reset {
                    self.state = LinkState::Available;
                    MasterEvent::LinkOk
                } else {
                    MasterEvent::Ack {
                        acd: false,
                        dfc: false,
                    }
                }
            }
            Frame::Fixed { control, address } => {
                if *address != self.params.address {
                    return MasterEvent::Unexpected {
                        reason: "链路地址不匹配".to_string(),
                    };
                }
                self.handle_secondary(*control, None)
            }
            Frame::Variable {
                control,
                address,
                asdu,
            } => {
                if *address != self.params.address {
                    return MasterEvent::Unexpected {
                        reason: "链路地址不匹配".to_string(),
                    };
                }
                self.handle_secondary(*control, Some(asdu.clone()))
            }
        }
    }

    /// 处理子站方向(PRM=0)的控制域。
    fn handle_secondary(&mut self, control: u8, asdu: Option<Vec<u8>>) -> MasterEvent {
        let cf = ControlField::parse(control);
        if cf.prm {
            return MasterEvent::Unexpected {
                reason: format!("收到主站方向帧,功能码 {}", cf.function),
            };
        }
        match cf.function {
            FC_ACK => {
                self.waiting_ack = false;
                self.last_acd = cf.acd;
                if self.state == LinkState::Busy {
                    self.state = LinkState::Available;
                }
                if self.state == LinkState::Reset {
                    self.state = LinkState::Available;
                    MasterEvent::LinkOk
                } else {
                    MasterEvent::Ack {
                        acd: cf.acd,
                        dfc: cf.dfc,
                    }
                }
            }
            FC_USER_DATA => {
                let asdu = match asdu {
                    Some(asdu) => asdu,
                    None => {
                        return MasterEvent::Unexpected {
                            reason: "用户数据帧缺少 ASDU".to_string(),
                        }
                    }
                };
                self.waiting_ack = false;
                self.last_acd = cf.acd;
                if self.state == LinkState::Busy {
                    self.state = LinkState::Available;
                }
                MasterEvent::Data {
                    asdu,
                    acd: cf.acd,
                    dfc: cf.dfc,
                }
            }
            FC_LINK_BUSY => {
                self.state = LinkState::Busy;
                MasterEvent::LinkBusy
            }
            FC_NAK => MasterEvent::Unexpected {
                reason: "收到否定认可".to_string(),
            },
            other => MasterEvent::Unexpected {
                reason: format!("未识别的子站功能码 {}", other),
            },
        }
    }

    /// 处理超时:等待用户数据确认时重发同一帧并保持 FCB 不变;连续超时达到
    /// `max_retries` 后把状态置回 [`LinkState::Reset`] 并返回 `None`。
    pub fn on_timeout(&mut self) -> Option<Frame> {
        if !self.waiting_ack {
            return None;
        }
        if self.retries >= self.params.max_retries {
            self.state = LinkState::Reset;
            self.waiting_ack = false;
            self.last_sent = None;
            return None;
        }
        self.retries += 1;
        self.last_sent.clone()
    }

    /// 把需要上送的用户数据排队。
    pub fn send_user_data(&mut self, asdu: Vec<u8>) {
        self.tx_queue.push_back(asdu);
    }

    /// 构造请求一级数据帧。
    fn request_primary(&self) -> Frame {
        Frame::Fixed {
            control: primary_control(FC_REQUEST_PRIMARY, false, false),
            address: self.params.address,
        }
    }

    /// 构造请求二级数据帧。
    fn request_secondary(&self) -> Frame {
        Frame::Fixed {
            control: primary_control(FC_REQUEST_SECONDARY, false, false),
            address: self.params.address,
        }
    }

    /// 构造请求链路状态帧。
    fn request_status(&self) -> Frame {
        Frame::Fixed {
            control: primary_control(FC_REQUEST_STATUS, false, false),
            address: self.params.address,
        }
    }
}
