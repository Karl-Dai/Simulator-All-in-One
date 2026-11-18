//! 101 应用层 ASDU 编解码（监视方向与控制方向）。
//!
//! ASDU 结构：
//!
//! ```text
//! 类型标识(1) | VSQ(1) | 传送原因(2) | 公共地址(CA) | 信息对象...
//! ```
//!
//! - VSQ：bit7 为 SQ，bit0-6 为信息对象数目。
//! - 传送原因：第一字节 bit7=T（试验）、bit6=P/N（否定确认）、bit0-5=原因值；
//!   第二字节为源发地址 OA。
//! - SQ=0：每个信息对象前带自己的信息对象地址 IOA。
//! - SQ=1：仅第一个信息对象带 IOA，后续地址按元素序号依次加 1。
//! - CA 长度 1 或 2 字节，IOA 长度 1、2 或 3 字节，均低字节在前。
//!
//! 品质位布局：
//!
//! - SIQ/DIQ：BL=bit4、SB=bit5、NT=bit6、IV=bit7。
//! - QDS：OV=bit0（本轮恒为 0，解码时忽略）、BL=bit1、SB=bit2、NT=bit3、
//!   IV=bit4，bit5-7 为 0。
//!
//! CP56Time2a 为 7 字节：毫秒小端、IV 位、SU 位、日、月、年。

/// 单点信息 M_SP_NA_1。
const M_SP_NA_1: u8 = 1;
/// 双点信息 M_DP_NA_1。
const M_DP_NA_1: u8 = 3;
/// 归一化测量值 M_ME_NA_1。
const M_ME_NA_1: u8 = 9;
/// 标度化测量值 M_ME_NB_1。
const M_ME_NB_1: u8 = 11;
/// 短浮点测量值 M_ME_NC_1。
const M_ME_NC_1: u8 = 13;
/// 带时标单点信息 M_SP_TB_1。
const M_SP_TB_1: u8 = 30;
/// 带时标双点信息 M_DP_TB_1。
const M_DP_TB_1: u8 = 31;
/// 带时标归一化测量值 M_ME_TD_1。
const M_ME_TD_1: u8 = 34;
/// 带时标标度化测量值 M_ME_TE_1。
const M_ME_TE_1: u8 = 35;
/// 带时标短浮点测量值 M_ME_TF_1。
const M_ME_TF_1: u8 = 36;
/// 单命令 C_SC_NA_1。
const C_SC_NA_1: u8 = 45;
/// 双命令 C_DC_NA_1。
const C_DC_NA_1: u8 = 46;
/// 步调节命令 C_RC_NA_1。
const C_RC_NA_1: u8 = 47;
/// 归一化设点 C_SE_NA_1。
const C_SE_NA_1: u8 = 48;
/// 标度化设点 C_SE_NB_1。
const C_SE_NB_1: u8 = 49;
/// 短浮点设点 C_SE_NC_1。
const C_SE_NC_1: u8 = 50;
/// 总召唤 C_IC_NA_1。
const C_IC_NA_1: u8 = 100;
/// 计数量召唤 C_CI_NA_1。
const C_CI_NA_1: u8 = 101;
/// 时钟同步 C_CS_NA_1。
const C_CS_NA_1: u8 = 103;
/// 复位进程 C_RP_NA_1。
const C_RP_NA_1: u8 = 105;

/// ASDU 地址配置。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AsduAddress {
    /// 公共地址。
    pub common_address: u16,
    /// 公共地址字节数，1 或 2。
    pub ca_len: u8,
    /// 信息对象地址字节数，1、2 或 3。
    pub ioa_len: u8,
}

/// 品质位。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Quality {
    /// 阻塞 BL。
    pub blocked: bool,
    /// 取代 SB。
    pub substituted: bool,
    /// 非当前值 NT。
    pub not_topical: bool,
    /// 无效 IV。
    pub invalid: bool,
}

/// CP56Time2a 时间。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cp56Time2a {
    /// 毫秒，0..=59999。
    pub milliseconds: u16,
    /// 分钟，0..=59。
    pub minute: u8,
    /// 小时，0..=23。
    pub hour: u8,
    /// 日，1..=31。
    pub day: u8,
    /// 月，1..=12。
    pub month: u8,
    /// 完整年份，例如 2026。
    pub year: u16,
    /// 无效 IV。
    pub invalid: bool,
    /// 夏令时 SU。
    pub summer_time: bool,
}

/// 信息对象。
#[derive(Debug, Clone, PartialEq)]
pub enum InformationObject {
    /// 单点信息。
    SinglePoint {
        /// 信息对象地址。
        ioa: u32,
        /// 单点值。
        value: bool,
        /// 品质位。
        quality: Quality,
    },
    /// 双点信息，`value` 取 0..=3。
    DoublePoint {
        /// 信息对象地址。
        ioa: u32,
        /// 双点值。
        value: u8,
        /// 品质位。
        quality: Quality,
    },
    /// 归一化测量值。
    Normalized {
        /// 信息对象地址。
        ioa: u32,
        /// 归一化值。
        value: i16,
        /// 品质位。
        quality: Quality,
    },
    /// 标度化测量值。
    Scaled {
        /// 信息对象地址。
        ioa: u32,
        /// 标度化值。
        value: i16,
        /// 品质位。
        quality: Quality,
    },
    /// 短浮点测量值。
    ShortFloat {
        /// 信息对象地址。
        ioa: u32,
        /// 短浮点值。
        value: f32,
        /// 品质位。
        quality: Quality,
    },
    /// 带时标单点信息。
    SinglePointTime {
        /// 信息对象地址。
        ioa: u32,
        /// 单点值。
        value: bool,
        /// 品质位。
        quality: Quality,
        /// 时标。
        time: Cp56Time2a,
    },
    /// 带时标双点信息。
    DoublePointTime {
        /// 信息对象地址。
        ioa: u32,
        /// 双点值。
        value: u8,
        /// 品质位。
        quality: Quality,
        /// 时标。
        time: Cp56Time2a,
    },
    /// 带时标归一化测量值。
    NormalizedTime {
        /// 信息对象地址。
        ioa: u32,
        /// 归一化值。
        value: i16,
        /// 品质位。
        quality: Quality,
        /// 时标。
        time: Cp56Time2a,
    },
    /// 带时标标度化测量值。
    ScaledTime {
        /// 信息对象地址。
        ioa: u32,
        /// 标度化值。
        value: i16,
        /// 品质位。
        quality: Quality,
        /// 时标。
        time: Cp56Time2a,
    },
    /// 带时标短浮点测量值。
    ShortFloatTime {
        /// 信息对象地址。
        ioa: u32,
        /// 短浮点值。
        value: f32,
        /// 品质位。
        quality: Quality,
        /// 时标。
        time: Cp56Time2a,
    },
    /// 单命令 C_SC_NA_1。
    SingleCommand {
        /// 信息对象地址。
        ioa: u32,
        /// SCS：单点命令状态，true 为合。
        on: bool,
        /// S/E：true 为选择，false 为执行。
        select: bool,
        /// QU 限定词，0..=3。
        qualifier: u8,
    },
    /// 双命令 C_DC_NA_1。
    DoubleCommand {
        /// 信息对象地址。
        ioa: u32,
        /// DCS：双点命令值，0..=3。
        value: u8,
        /// S/E：true 为选择，false 为执行。
        select: bool,
        /// QU 限定词，0..=3。
        qualifier: u8,
    },
    /// 步调节命令 C_RC_NA_1。
    StepCommand {
        /// 信息对象地址。
        ioa: u32,
        /// RCS：步调节值，0..=31。
        value: u8,
        /// S/E：true 为选择，false 为执行。
        select: bool,
        /// QU 限定词，0..=3。
        qualifier: u8,
    },
    /// 归一化设点 C_SE_NA_1。
    SetpointNormalized {
        /// 信息对象地址。
        ioa: u32,
        /// 归一化设点值。
        value: i16,
        /// S/E：true 为选择，false 为执行。
        select: bool,
        /// QL 限定词，0..=127。
        qualifier: u8,
    },
    /// 标度化设点 C_SE_NB_1。
    SetpointScaled {
        /// 信息对象地址。
        ioa: u32,
        /// 标度化设点值。
        value: i16,
        /// S/E：true 为选择，false 为执行。
        select: bool,
        /// QL 限定词，0..=127。
        qualifier: u8,
    },
    /// 短浮点设点 C_SE_NC_1。
    SetpointFloat {
        /// 信息对象地址。
        ioa: u32,
        /// 短浮点设点值。
        value: f32,
        /// S/E：true 为选择，false 为执行。
        select: bool,
        /// QL 限定词，0..=127。
        qualifier: u8,
    },
    /// 总召唤 C_IC_NA_1。
    InterrogationCommand {
        /// 信息对象地址。
        ioa: u32,
        /// QOI 召唤限定词。
        qoi: u8,
    },
    /// 计数量召唤 C_CI_NA_1。
    CounterInterrogation {
        /// 信息对象地址。
        ioa: u32,
        /// QCC 计数量召唤限定词。
        qcc: u8,
    },
    /// 时钟同步 C_CS_NA_1。
    ClockSync {
        /// 信息对象地址。
        ioa: u32,
        /// 时标。
        time: Cp56Time2a,
    },
    /// 复位进程 C_RP_NA_1。
    ResetProcess {
        /// 信息对象地址。
        ioa: u32,
        /// QRP 复位限定词。
        qrp: u8,
    },
}

impl InformationObject {
    /// 返回信息对象地址。
    pub fn ioa(&self) -> u32 {
        match self {
            Self::SinglePoint { ioa, .. }
            | Self::DoublePoint { ioa, .. }
            | Self::Normalized { ioa, .. }
            | Self::Scaled { ioa, .. }
            | Self::ShortFloat { ioa, .. }
            | Self::SinglePointTime { ioa, .. }
            | Self::DoublePointTime { ioa, .. }
            | Self::NormalizedTime { ioa, .. }
            | Self::ScaledTime { ioa, .. }
            | Self::ShortFloatTime { ioa, .. }
            | Self::SingleCommand { ioa, .. }
            | Self::DoubleCommand { ioa, .. }
            | Self::StepCommand { ioa, .. }
            | Self::SetpointNormalized { ioa, .. }
            | Self::SetpointScaled { ioa, .. }
            | Self::SetpointFloat { ioa, .. }
            | Self::InterrogationCommand { ioa, .. }
            | Self::CounterInterrogation { ioa, .. }
            | Self::ClockSync { ioa, .. }
            | Self::ResetProcess { ioa, .. } => *ioa,
        }
    }
}

/// ASDU。
#[derive(Debug, Clone, PartialEq)]
pub struct Asdu {
    /// 类型标识。
    pub type_id: u8,
    /// SQ 位。
    pub sq: bool,
    /// 传送原因值，0..=63。
    pub cause: u8,
    /// 否定确认 P/N。
    pub negative: bool,
    /// 试验 T。
    pub test: bool,
    /// 源发地址 OA。
    pub originator: u8,
    /// 地址配置。
    pub address: AsduAddress,
    /// 信息对象列表。
    pub objects: Vec<InformationObject>,
}

/// ASDU 编解码错误。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsduError {
    /// 数据不完整。
    TooShort,
    /// 未知类型标识。
    UnknownType(u8),
    /// 信息对象数目为 0。
    ObjectCountMismatch,
    /// 长度不匹配，`expected` 为所需字节数，`actual` 为实际字节数。
    LengthMismatch {
        /// 所需字节数。
        expected: usize,
        /// 实际字节数。
        actual: usize,
    },
    /// 地址长度非法。
    UnsupportedAddressLength,
    /// 限定词非法，信息对象类型与类型标识不匹配。
    InvalidQualifier,
    /// 值超出范围。
    ValueOutOfRange,
}

/// 返回类型标识对应的信息元素字节数（不含 IOA）。
fn element_size(type_id: u8) -> Option<usize> {
    match type_id {
        M_SP_NA_1 | M_DP_NA_1 => Some(1),
        M_ME_NA_1 | M_ME_NB_1 => Some(3),
        M_ME_NC_1 => Some(5),
        M_SP_TB_1 | M_DP_TB_1 => Some(8),
        M_ME_TD_1 | M_ME_TE_1 => Some(10),
        M_ME_TF_1 => Some(12),
        C_SC_NA_1 | C_DC_NA_1 | C_RC_NA_1 => Some(1),
        C_SE_NA_1 | C_SE_NB_1 => Some(3),
        C_SE_NC_1 => Some(5),
        C_IC_NA_1 | C_CI_NA_1 | C_RP_NA_1 => Some(1),
        C_CS_NA_1 => Some(7),
        _ => None,
    }
}

/// 校验 CA 长度。
fn validate_ca_len(len: u8) -> Result<usize, AsduError> {
    match len {
        1..=2 => Ok(len as usize),
        _ => Err(AsduError::UnsupportedAddressLength),
    }
}

/// 校验 IOA 长度。
fn validate_ioa_len(len: u8) -> Result<usize, AsduError> {
    match len {
        1..=3 => Ok(len as usize),
        _ => Err(AsduError::UnsupportedAddressLength),
    }
}

/// 返回地址长度允许的最大值。
fn address_limit(len: usize) -> u32 {
    match len {
        1 => 0xFF,
        2 => 0xFFFF,
        3 => 0x00FF_FFFF,
        _ => 0,
    }
}

/// 按低字节在前写入地址。
fn push_address(out: &mut Vec<u8>, value: u32, len: usize) {
    for i in 0..len {
        out.push(((value >> (8 * i)) & 0xFF) as u8);
    }
}

/// 按低字节在前读取地址。
fn read_address(bytes: &[u8]) -> u32 {
    let mut value = 0u32;
    for (i, &b) in bytes.iter().enumerate() {
        value |= (b as u32) << (8 * i);
    }
    value
}

/// 从 SIQ/DIQ 字节解析品质位。
fn quality_from_siq(byte: u8) -> Quality {
    Quality {
        blocked: byte & 0x10 != 0,
        substituted: byte & 0x20 != 0,
        not_topical: byte & 0x40 != 0,
        invalid: byte & 0x80 != 0,
    }
}

/// 将品质位写入 SIQ/DIQ 字节。
fn quality_siq(quality: &Quality) -> u8 {
    (if quality.blocked { 0x10 } else { 0 })
        | (if quality.substituted { 0x20 } else { 0 })
        | (if quality.not_topical { 0x40 } else { 0 })
        | (if quality.invalid { 0x80 } else { 0 })
}

/// 从 QDS 字节解析品质位，OV 位忽略。
fn quality_from_qds(byte: u8) -> Quality {
    Quality {
        blocked: byte & 0x02 != 0,
        substituted: byte & 0x04 != 0,
        not_topical: byte & 0x08 != 0,
        invalid: byte & 0x10 != 0,
    }
}

/// 将品质位写入 QDS 字节，OV 位恒为 0。
fn quality_qds(quality: &Quality) -> u8 {
    (if quality.blocked { 0x02 } else { 0 })
        | (if quality.substituted { 0x04 } else { 0 })
        | (if quality.not_topical { 0x08 } else { 0 })
        | (if quality.invalid { 0x10 } else { 0 })
}

/// 编码控制命令字节（SCO/DCO/RCO）。
///
/// bit7 为 S/E，bit5-6 为 QU，bit0-4 为命令值。
fn control_byte(select: bool, qualifier: u8, value: u8) -> u8 {
    ((select as u8) << 7) | ((qualifier & 0x03) << 5) | (value & 0x1F)
}

/// 编码 QOS 字节。
///
/// bit7 为 S/E，bit0-6 为 QL。
fn qos_byte(select: bool, qualifier: u8) -> u8 {
    ((select as u8) << 7) | (qualifier & 0x7F)
}

/// 把切片复制成 7 字节数组。调用方需保证切片长度至少为 7。
fn seven(bytes: &[u8]) -> [u8; 7] {
    [
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6],
    ]
}

/// 编码 CP56Time2a（7 字节）。
///
/// 布局与 104 仿真器保持一致：
/// - b0..b1：毫秒，低字节在前；
/// - b2：IV(bit7) | 分钟(bit0-5)；
/// - b3：SU(bit7) | 小时(bit0-4)；
/// - b4：星期(bit5-7) | 日(bit0-4)，本结构无星期，高 3 位为 0；
/// - b5：月(bit0-3)；
/// - b6：年减去 2000，取低 7 位。
pub fn encode_cp56time2a(time: &Cp56Time2a) -> [u8; 7] {
    let ms = time.milliseconds.min(59999);
    [
        (ms & 0xFF) as u8,
        ((ms >> 8) & 0xFF) as u8,
        (time.minute & 0x3F) | if time.invalid { 0x80 } else { 0 },
        (time.hour & 0x1F) | if time.summer_time { 0x80 } else { 0 },
        time.day & 0x1F,
        time.month & 0x0F,
        (time.year.wrapping_sub(2000) as u8) & 0x7F,
    ]
}

/// 解码 CP56Time2a（7 字节）。
pub fn decode_cp56time2a(bytes: &[u8; 7]) -> Cp56Time2a {
    Cp56Time2a {
        milliseconds: u16::from_le_bytes([bytes[0], bytes[1]]),
        minute: bytes[2] & 0x3F,
        hour: bytes[3] & 0x1F,
        day: bytes[4] & 0x1F,
        month: bytes[5] & 0x0F,
        year: 2000 + (bytes[6] & 0x7F) as u16,
        invalid: bytes[2] & 0x80 != 0,
        summer_time: bytes[3] & 0x80 != 0,
    }
}

/// 校验时间字段范围并编码。
fn encode_time_checked(time: &Cp56Time2a) -> Result<[u8; 7], AsduError> {
    if time.milliseconds > 59999
        || time.minute > 59
        || time.hour > 23
        || time.day == 0
        || time.day > 31
        || time.month == 0
        || time.month > 12
    {
        return Err(AsduError::ValueOutOfRange);
    }
    Ok(encode_cp56time2a(time))
}

/// 编码一个信息对象的元素部分（不含 IOA）。
fn encode_element(
    out: &mut Vec<u8>,
    type_id: u8,
    object: &InformationObject,
) -> Result<(), AsduError> {
    match object {
        InformationObject::SinglePoint { value, quality, .. } => {
            if type_id != M_SP_NA_1 {
                return Err(AsduError::InvalidQualifier);
            }
            out.push((*value as u8) | quality_siq(quality));
        }
        InformationObject::SinglePointTime {
            value,
            quality,
            time,
            ..
        } => {
            if type_id != M_SP_TB_1 {
                return Err(AsduError::InvalidQualifier);
            }
            out.push((*value as u8) | quality_siq(quality));
            out.extend_from_slice(&encode_time_checked(time)?);
        }
        InformationObject::DoublePoint { value, quality, .. } => {
            if type_id != M_DP_NA_1 {
                return Err(AsduError::InvalidQualifier);
            }
            if *value > 0x03 {
                return Err(AsduError::ValueOutOfRange);
            }
            out.push((*value & 0x03) | quality_siq(quality));
        }
        InformationObject::DoublePointTime {
            value,
            quality,
            time,
            ..
        } => {
            if type_id != M_DP_TB_1 {
                return Err(AsduError::InvalidQualifier);
            }
            if *value > 0x03 {
                return Err(AsduError::ValueOutOfRange);
            }
            out.push((*value & 0x03) | quality_siq(quality));
            out.extend_from_slice(&encode_time_checked(time)?);
        }
        InformationObject::Normalized { value, quality, .. } => {
            if type_id != M_ME_NA_1 {
                return Err(AsduError::InvalidQualifier);
            }
            out.extend_from_slice(&value.to_le_bytes());
            out.push(quality_qds(quality));
        }
        InformationObject::NormalizedTime {
            value,
            quality,
            time,
            ..
        } => {
            if type_id != M_ME_TD_1 {
                return Err(AsduError::InvalidQualifier);
            }
            out.extend_from_slice(&value.to_le_bytes());
            out.push(quality_qds(quality));
            out.extend_from_slice(&encode_time_checked(time)?);
        }
        InformationObject::Scaled { value, quality, .. } => {
            if type_id != M_ME_NB_1 {
                return Err(AsduError::InvalidQualifier);
            }
            out.extend_from_slice(&value.to_le_bytes());
            out.push(quality_qds(quality));
        }
        InformationObject::ScaledTime {
            value,
            quality,
            time,
            ..
        } => {
            if type_id != M_ME_TE_1 {
                return Err(AsduError::InvalidQualifier);
            }
            out.extend_from_slice(&value.to_le_bytes());
            out.push(quality_qds(quality));
            out.extend_from_slice(&encode_time_checked(time)?);
        }
        InformationObject::ShortFloat { value, quality, .. } => {
            if type_id != M_ME_NC_1 {
                return Err(AsduError::InvalidQualifier);
            }
            out.extend_from_slice(&value.to_le_bytes());
            out.push(quality_qds(quality));
        }
        InformationObject::ShortFloatTime {
            value,
            quality,
            time,
            ..
        } => {
            if type_id != M_ME_TF_1 {
                return Err(AsduError::InvalidQualifier);
            }
            out.extend_from_slice(&value.to_le_bytes());
            out.push(quality_qds(quality));
            out.extend_from_slice(&encode_time_checked(time)?);
        }
        InformationObject::SingleCommand {
            on,
            select,
            qualifier,
            ..
        } => {
            if type_id != C_SC_NA_1 {
                return Err(AsduError::InvalidQualifier);
            }
            if *qualifier > 0x03 {
                return Err(AsduError::ValueOutOfRange);
            }
            out.push(control_byte(*select, *qualifier, *on as u8));
        }
        InformationObject::DoubleCommand {
            value,
            select,
            qualifier,
            ..
        } => {
            if type_id != C_DC_NA_1 {
                return Err(AsduError::InvalidQualifier);
            }
            if *value > 0x03 || *qualifier > 0x03 {
                return Err(AsduError::ValueOutOfRange);
            }
            out.push(control_byte(*select, *qualifier, *value));
        }
        InformationObject::StepCommand {
            value,
            select,
            qualifier,
            ..
        } => {
            if type_id != C_RC_NA_1 {
                return Err(AsduError::InvalidQualifier);
            }
            if *value > 0x1F || *qualifier > 0x03 {
                return Err(AsduError::ValueOutOfRange);
            }
            out.push(control_byte(*select, *qualifier, *value));
        }
        InformationObject::SetpointNormalized {
            value,
            select,
            qualifier,
            ..
        } => {
            if type_id != C_SE_NA_1 {
                return Err(AsduError::InvalidQualifier);
            }
            if *qualifier > 0x7F {
                return Err(AsduError::ValueOutOfRange);
            }
            out.extend_from_slice(&value.to_le_bytes());
            out.push(qos_byte(*select, *qualifier));
        }
        InformationObject::SetpointScaled {
            value,
            select,
            qualifier,
            ..
        } => {
            if type_id != C_SE_NB_1 {
                return Err(AsduError::InvalidQualifier);
            }
            if *qualifier > 0x7F {
                return Err(AsduError::ValueOutOfRange);
            }
            out.extend_from_slice(&value.to_le_bytes());
            out.push(qos_byte(*select, *qualifier));
        }
        InformationObject::SetpointFloat {
            value,
            select,
            qualifier,
            ..
        } => {
            if type_id != C_SE_NC_1 {
                return Err(AsduError::InvalidQualifier);
            }
            if *qualifier > 0x7F {
                return Err(AsduError::ValueOutOfRange);
            }
            out.extend_from_slice(&value.to_le_bytes());
            out.push(qos_byte(*select, *qualifier));
        }
        InformationObject::InterrogationCommand { qoi, .. } => {
            if type_id != C_IC_NA_1 {
                return Err(AsduError::InvalidQualifier);
            }
            out.push(*qoi);
        }
        InformationObject::CounterInterrogation { qcc, .. } => {
            if type_id != C_CI_NA_1 {
                return Err(AsduError::InvalidQualifier);
            }
            out.push(*qcc);
        }
        InformationObject::ClockSync { time, .. } => {
            if type_id != C_CS_NA_1 {
                return Err(AsduError::InvalidQualifier);
            }
            out.extend_from_slice(&encode_time_checked(time)?);
        }
        InformationObject::ResetProcess { qrp, .. } => {
            if type_id != C_RP_NA_1 {
                return Err(AsduError::InvalidQualifier);
            }
            out.push(*qrp);
        }
    }
    Ok(())
}

/// 编码 ASDU。
///
/// 信息对象数目必须大于 0 且不超过 127；双点值必须 0..=3；时间字段必须在
/// 结构体注释给出的范围内；地址不能超过地址长度允许的最大值。越界返回
/// [`AsduError::ValueOutOfRange`]。
pub fn encode_asdu(asdu: &Asdu) -> Result<Vec<u8>, AsduError> {
    let ca_len = validate_ca_len(asdu.address.ca_len)?;
    let ioa_len = validate_ioa_len(asdu.address.ioa_len)?;
    let elem_size = element_size(asdu.type_id).ok_or(AsduError::UnknownType(asdu.type_id))?;

    let count = asdu.objects.len();
    if count == 0 {
        return Err(AsduError::ObjectCountMismatch);
    }
    if count > 0x7F {
        return Err(AsduError::ValueOutOfRange);
    }
    if asdu.cause > 0x3F {
        return Err(AsduError::ValueOutOfRange);
    }
    if asdu.address.common_address as u32 > address_limit(ca_len) {
        return Err(AsduError::ValueOutOfRange);
    }
    let ioa_limit = address_limit(ioa_len);

    let mut out = Vec::with_capacity(4 + ca_len + count * (ioa_len + elem_size));
    out.push(asdu.type_id);
    out.push(((asdu.sq as u8) << 7) | (count as u8));
    out.push(((asdu.test as u8) << 7) | ((asdu.negative as u8) << 6) | (asdu.cause & 0x3F));
    out.push(asdu.originator);
    push_address(&mut out, asdu.address.common_address as u32, ca_len);

    if asdu.sq {
        let base = asdu.objects[0].ioa();
        if base > ioa_limit {
            return Err(AsduError::ValueOutOfRange);
        }
        let last = base
            .checked_add(count as u32 - 1)
            .ok_or(AsduError::ValueOutOfRange)?;
        if last > ioa_limit {
            return Err(AsduError::ValueOutOfRange);
        }
        push_address(&mut out, base, ioa_len);
        for object in &asdu.objects {
            encode_element(&mut out, asdu.type_id, object)?;
        }
    } else {
        for object in &asdu.objects {
            let ioa = object.ioa();
            if ioa > ioa_limit {
                return Err(AsduError::ValueOutOfRange);
            }
            push_address(&mut out, ioa, ioa_len);
            encode_element(&mut out, asdu.type_id, object)?;
        }
    }
    Ok(out)
}

/// 解码一个信息对象的元素部分（不含 IOA），返回对象与消费的字节数。
fn decode_element(
    type_id: u8,
    ioa: u32,
    bytes: &[u8],
) -> Result<(InformationObject, usize), AsduError> {
    match type_id {
        M_SP_NA_1 => {
            if bytes.is_empty() {
                return Err(AsduError::TooShort);
            }
            let siq = bytes[0];
            Ok((
                InformationObject::SinglePoint {
                    ioa,
                    value: siq & 0x01 != 0,
                    quality: quality_from_siq(siq),
                },
                1,
            ))
        }
        M_DP_NA_1 => {
            if bytes.is_empty() {
                return Err(AsduError::TooShort);
            }
            let diq = bytes[0];
            Ok((
                InformationObject::DoublePoint {
                    ioa,
                    value: diq & 0x03,
                    quality: quality_from_siq(diq),
                },
                1,
            ))
        }
        M_ME_NA_1 => {
            if bytes.len() < 3 {
                return Err(AsduError::TooShort);
            }
            Ok((
                InformationObject::Normalized {
                    ioa,
                    value: i16::from_le_bytes([bytes[0], bytes[1]]),
                    quality: quality_from_qds(bytes[2]),
                },
                3,
            ))
        }
        M_ME_NB_1 => {
            if bytes.len() < 3 {
                return Err(AsduError::TooShort);
            }
            Ok((
                InformationObject::Scaled {
                    ioa,
                    value: i16::from_le_bytes([bytes[0], bytes[1]]),
                    quality: quality_from_qds(bytes[2]),
                },
                3,
            ))
        }
        M_ME_NC_1 => {
            if bytes.len() < 5 {
                return Err(AsduError::TooShort);
            }
            Ok((
                InformationObject::ShortFloat {
                    ioa,
                    value: f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
                    quality: quality_from_qds(bytes[4]),
                },
                5,
            ))
        }
        M_SP_TB_1 => {
            if bytes.len() < 8 {
                return Err(AsduError::TooShort);
            }
            let siq = bytes[0];
            let time = decode_cp56time2a(&seven(&bytes[1..8]));
            Ok((
                InformationObject::SinglePointTime {
                    ioa,
                    value: siq & 0x01 != 0,
                    quality: quality_from_siq(siq),
                    time,
                },
                8,
            ))
        }
        M_DP_TB_1 => {
            if bytes.len() < 8 {
                return Err(AsduError::TooShort);
            }
            let diq = bytes[0];
            let time = decode_cp56time2a(&seven(&bytes[1..8]));
            Ok((
                InformationObject::DoublePointTime {
                    ioa,
                    value: diq & 0x03,
                    quality: quality_from_siq(diq),
                    time,
                },
                8,
            ))
        }
        M_ME_TD_1 => {
            if bytes.len() < 10 {
                return Err(AsduError::TooShort);
            }
            let value = i16::from_le_bytes([bytes[0], bytes[1]]);
            let quality = quality_from_qds(bytes[2]);
            let time = decode_cp56time2a(&seven(&bytes[3..10]));
            Ok((
                InformationObject::NormalizedTime {
                    ioa,
                    value,
                    quality,
                    time,
                },
                10,
            ))
        }
        M_ME_TE_1 => {
            if bytes.len() < 10 {
                return Err(AsduError::TooShort);
            }
            let value = i16::from_le_bytes([bytes[0], bytes[1]]);
            let quality = quality_from_qds(bytes[2]);
            let time = decode_cp56time2a(&seven(&bytes[3..10]));
            Ok((
                InformationObject::ScaledTime {
                    ioa,
                    value,
                    quality,
                    time,
                },
                10,
            ))
        }
        M_ME_TF_1 => {
            if bytes.len() < 12 {
                return Err(AsduError::TooShort);
            }
            let value = f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
            let quality = quality_from_qds(bytes[4]);
            let time = decode_cp56time2a(&seven(&bytes[5..12]));
            Ok((
                InformationObject::ShortFloatTime {
                    ioa,
                    value,
                    quality,
                    time,
                },
                12,
            ))
        }
        C_SC_NA_1 => {
            if bytes.is_empty() {
                return Err(AsduError::TooShort);
            }
            let b = bytes[0];
            Ok((
                InformationObject::SingleCommand {
                    ioa,
                    on: b & 0x01 != 0,
                    select: b & 0x80 != 0,
                    qualifier: (b >> 5) & 0x03,
                },
                1,
            ))
        }
        C_DC_NA_1 => {
            if bytes.is_empty() {
                return Err(AsduError::TooShort);
            }
            let b = bytes[0];
            Ok((
                InformationObject::DoubleCommand {
                    ioa,
                    value: b & 0x03,
                    select: b & 0x80 != 0,
                    qualifier: (b >> 5) & 0x03,
                },
                1,
            ))
        }
        C_RC_NA_1 => {
            if bytes.is_empty() {
                return Err(AsduError::TooShort);
            }
            let b = bytes[0];
            Ok((
                InformationObject::StepCommand {
                    ioa,
                    value: b & 0x1F,
                    select: b & 0x80 != 0,
                    qualifier: (b >> 5) & 0x03,
                },
                1,
            ))
        }
        C_SE_NA_1 => {
            if bytes.len() < 3 {
                return Err(AsduError::TooShort);
            }
            Ok((
                InformationObject::SetpointNormalized {
                    ioa,
                    value: i16::from_le_bytes([bytes[0], bytes[1]]),
                    select: bytes[2] & 0x80 != 0,
                    qualifier: bytes[2] & 0x7F,
                },
                3,
            ))
        }
        C_SE_NB_1 => {
            if bytes.len() < 3 {
                return Err(AsduError::TooShort);
            }
            Ok((
                InformationObject::SetpointScaled {
                    ioa,
                    value: i16::from_le_bytes([bytes[0], bytes[1]]),
                    select: bytes[2] & 0x80 != 0,
                    qualifier: bytes[2] & 0x7F,
                },
                3,
            ))
        }
        C_SE_NC_1 => {
            if bytes.len() < 5 {
                return Err(AsduError::TooShort);
            }
            Ok((
                InformationObject::SetpointFloat {
                    ioa,
                    value: f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
                    select: bytes[4] & 0x80 != 0,
                    qualifier: bytes[4] & 0x7F,
                },
                5,
            ))
        }
        C_IC_NA_1 => {
            if bytes.is_empty() {
                return Err(AsduError::TooShort);
            }
            Ok((
                InformationObject::InterrogationCommand { ioa, qoi: bytes[0] },
                1,
            ))
        }
        C_CI_NA_1 => {
            if bytes.is_empty() {
                return Err(AsduError::TooShort);
            }
            Ok((
                InformationObject::CounterInterrogation { ioa, qcc: bytes[0] },
                1,
            ))
        }
        C_CS_NA_1 => {
            if bytes.len() < 7 {
                return Err(AsduError::TooShort);
            }
            Ok((
                InformationObject::ClockSync {
                    ioa,
                    time: decode_cp56time2a(&seven(bytes)),
                },
                7,
            ))
        }
        C_RP_NA_1 => {
            if bytes.is_empty() {
                return Err(AsduError::TooShort);
            }
            Ok((InformationObject::ResetProcess { ioa, qrp: bytes[0] }, 1))
        }
        _ => Err(AsduError::UnknownType(type_id)),
    }
}

/// 解码 ASDU，返回 ASDU 与已消费的字节数。
///
/// `bytes` 可以包含 ASDU 之后的额外数据，已消费字节数表示 ASDU 结束位置。
/// 缓冲区短于固定头部时返回 [`AsduError::TooShort`]；头部可解析但声明的
/// 信息对象放不进缓冲区时返回 [`AsduError::LengthMismatch`]。
pub fn decode_asdu(bytes: &[u8], address: AsduAddress) -> Result<(Asdu, usize), AsduError> {
    let ca_len = validate_ca_len(address.ca_len)?;
    let ioa_len = validate_ioa_len(address.ioa_len)?;
    let header = 4 + ca_len;
    if bytes.len() < header {
        return Err(AsduError::TooShort);
    }

    let type_id = bytes[0];
    let vsq = bytes[1];
    let sq = vsq & 0x80 != 0;
    let count = (vsq & 0x7F) as usize;
    let elem_size = element_size(type_id).ok_or(AsduError::UnknownType(type_id))?;
    if count == 0 {
        return Err(AsduError::ObjectCountMismatch);
    }

    let cause_byte = bytes[2];
    let test = cause_byte & 0x80 != 0;
    let negative = cause_byte & 0x40 != 0;
    let cause = cause_byte & 0x3F;
    let originator = bytes[3];
    let common_address = read_address(&bytes[4..4 + ca_len]) as u16;
    let decoded_address = AsduAddress {
        common_address,
        ca_len: address.ca_len,
        ioa_len: address.ioa_len,
    };

    let expected = if sq {
        header + ioa_len + count * elem_size
    } else {
        header + count * (ioa_len + elem_size)
    };
    if bytes.len() < expected {
        return Err(AsduError::LengthMismatch {
            expected,
            actual: bytes.len(),
        });
    }

    let mut objects = Vec::with_capacity(count);
    let mut pos = header;
    if sq {
        let base = read_address(&bytes[pos..pos + ioa_len]);
        pos += ioa_len;
        for index in 0..count {
            let ioa = base
                .checked_add(index as u32)
                .ok_or(AsduError::ValueOutOfRange)?;
            let (object, consumed) = decode_element(type_id, ioa, &bytes[pos..])?;
            pos += consumed;
            objects.push(object);
        }
    } else {
        for _ in 0..count {
            let ioa = read_address(&bytes[pos..pos + ioa_len]);
            pos += ioa_len;
            let (object, consumed) = decode_element(type_id, ioa, &bytes[pos..])?;
            pos += consumed;
            objects.push(object);
        }
    }

    Ok((
        Asdu {
            type_id,
            sq,
            cause,
            negative,
            test,
            originator,
            address: decoded_address,
            objects,
        },
        pos,
    ))
}
