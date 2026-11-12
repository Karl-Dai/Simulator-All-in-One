//! 102 应用层累计量 ASDU 编解码。
//!
//! ASDU 结构与 101 相同：
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
//! 本轮支持的类型：
//!
//! - 100 M_IT_NA_1：BCR（5 字节）。
//! - 101 M_IT_TA_1：BCR + CP24Time2a（3 字节）。
//! - 103 M_IT_TC_1：BCR + CP56Time2a（7 字节）。
//! - 120 C_IT_NA_1：QCC（1 字节）。
//! - 121 C_IT_TA_1：QCC + CP24Time2a（3 字节）。
//! - 123 C_IT_TC_1：QCC + CP56Time2a（7 字节）。
//!
//! 类型 102 M_IT_TB_1 暂不实现：该类型使用 CP40Time2a 时标，其位定义需要按
//! 标准原文核对后才能编码。解码遇到类型 102 时返回
//! [`Asdu102Error::UnsupportedType`]，[`InformationObject102`] 也不提供对应
//! 变体。
//!
//! BCR 布局：
//!
//! - 第 1-4 字节：二进制计数器读数，u32，小端。
//! - 第 5 字节：bit0-4 顺序号 SEQ，bit5 进位 CY，bit6 计数器被调整 CA，
//!   bit7 无效 IV。
//!
//! QCC 布局：bit0-5 请求分组 RQT，bit6-7 冻结方式 FRZ。
//!
//! CP24Time2a 为 3 字节：毫秒低字节、毫秒高字节、IV(bit7) 与分钟(bit0-5)。
//! CP56Time2a 为 7 字节，位布局与 101 相同。

/// 累计量信息 M_IT_NA_1（类型 100）。
const M_IT_NA_1: u8 = 100;
/// 带 CP24Time2a 的累计量信息 M_IT_TA_1（类型 101）。
const M_IT_TA_1: u8 = 101;
/// 带 CP56Time2a 的累计量信息 M_IT_TC_1（类型 103）。
const M_IT_TC_1: u8 = 103;
/// 累计量召唤 C_IT_NA_1（类型 120）。
const C_IT_NA_1: u8 = 120;
/// 带 CP24Time2a 的累计量召唤 C_IT_TA_1（类型 121）。
const C_IT_TA_1: u8 = 121;
/// 带 CP56Time2a 的累计量召唤 C_IT_TC_1（类型 123）。
const C_IT_TC_1: u8 = 123;

/// ASDU 地址配置。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StationAddress {
    /// 公共地址。
    pub common_address: u16,
    /// 公共地址字节数，1 或 2。
    pub ca_len: u8,
    /// 信息对象地址字节数，1、2 或 3。
    pub ioa_len: u8,
}

/// 二进制累计量读数（BCR）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BinaryCounter {
    /// 二进制计数器读数。
    pub reading: u32,
    /// 顺序号 SEQ，取值 0..=31。
    pub sequence: u8,
    /// 进位 CY。
    pub carry: bool,
    /// 计数器被调整 CA。
    pub adjusted: bool,
    /// 无效 IV。
    pub invalid: bool,
}

/// CP24Time2a 时标（3 字节）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cp24Time2a {
    /// 毫秒，0..=59999。
    pub milliseconds: u16,
    /// 分钟，0..=59。
    pub minute: u8,
    /// 无效 IV。
    pub invalid: bool,
}

/// CP56Time2a 时标（7 字节）。
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

/// 累计量召唤限定词（QCC）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Qcc {
    /// 请求分组 RQT，0 表示全局，1..=4 表示分组。
    pub request: u8,
    /// 冻结方式 FRZ：0 读，1 冻结并读，2 冻结并复位读，3 复位读。
    pub freeze: u8,
}

/// 累计量信息对象。
#[derive(Debug, Clone, PartialEq)]
pub enum InformationObject102 {
    /// 类型 100：累计量读数。
    Counter {
        /// 信息对象地址。
        ioa: u32,
        /// 累计量读数。
        counter: BinaryCounter,
    },
    /// 类型 101：带 CP24Time2a 的累计量读数。
    CounterCp24 {
        /// 信息对象地址。
        ioa: u32,
        /// 累计量读数。
        counter: BinaryCounter,
        /// 时标。
        time: Cp24Time2a,
    },
    /// 类型 103：带 CP56Time2a 的累计量读数。
    CounterCp56 {
        /// 信息对象地址。
        ioa: u32,
        /// 累计量读数。
        counter: BinaryCounter,
        /// 时标。
        time: Cp56Time2a,
    },
    /// 类型 120：累计量召唤。
    CounterInterrogation {
        /// 信息对象地址。
        ioa: u32,
        /// 累计量召唤限定词。
        qcc: Qcc,
    },
    /// 类型 121：带 CP24Time2a 的累计量召唤。
    CounterInterrogationCp24 {
        /// 信息对象地址。
        ioa: u32,
        /// 累计量召唤限定词。
        qcc: Qcc,
        /// 时标。
        time: Cp24Time2a,
    },
    /// 类型 123：带 CP56Time2a 的累计量召唤。
    CounterInterrogationCp56 {
        /// 信息对象地址。
        ioa: u32,
        /// 累计量召唤限定词。
        qcc: Qcc,
        /// 时标。
        time: Cp56Time2a,
    },
}

impl InformationObject102 {
    /// 返回信息对象地址。
    pub fn ioa(&self) -> u32 {
        match self {
            Self::Counter { ioa, .. }
            | Self::CounterCp24 { ioa, .. }
            | Self::CounterCp56 { ioa, .. }
            | Self::CounterInterrogation { ioa, .. }
            | Self::CounterInterrogationCp24 { ioa, .. }
            | Self::CounterInterrogationCp56 { ioa, .. } => *ioa,
        }
    }
}

/// 102 ASDU。
#[derive(Debug, Clone, PartialEq)]
pub struct Asdu102 {
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
    pub address: StationAddress,
    /// 信息对象列表。
    pub objects: Vec<InformationObject102>,
}

/// ASDU 编解码错误。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Asdu102Error {
    /// 数据不完整。
    TooShort,
    /// 未知或暂不支持的类型标识。
    UnsupportedType(u8),
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
    /// 值超出范围。
    ValueOutOfRange,
    /// 信息对象类型与类型标识不匹配。
    TypeMismatch,
}

/// 返回类型标识对应的信息元素字节数（不含 IOA）。
fn element_size(type_id: u8) -> Option<usize> {
    match type_id {
        M_IT_NA_1 => Some(5),
        M_IT_TA_1 => Some(8),
        M_IT_TC_1 => Some(12),
        C_IT_NA_1 => Some(1),
        C_IT_TA_1 => Some(4),
        C_IT_TC_1 => Some(8),
        _ => None,
    }
}

/// 校验 CA 长度。
fn validate_ca_len(len: u8) -> Result<usize, Asdu102Error> {
    match len {
        1..=2 => Ok(len as usize),
        _ => Err(Asdu102Error::UnsupportedAddressLength),
    }
}

/// 校验 IOA 长度。
fn validate_ioa_len(len: u8) -> Result<usize, Asdu102Error> {
    match len {
        1..=3 => Ok(len as usize),
        _ => Err(Asdu102Error::UnsupportedAddressLength),
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

/// 编码 BCR（5 字节）。
///
/// 顺序号必须 0..=31，否则返回 [`Asdu102Error::ValueOutOfRange`]。
pub fn encode_bcr(counter: &BinaryCounter) -> Result<[u8; 5], Asdu102Error> {
    if counter.sequence > 0x1F {
        return Err(Asdu102Error::ValueOutOfRange);
    }
    let mut out = [0u8; 5];
    out[0..4].copy_from_slice(&counter.reading.to_le_bytes());
    out[4] = (counter.sequence & 0x1F)
        | (if counter.carry { 0x20 } else { 0 })
        | (if counter.adjusted { 0x40 } else { 0 })
        | (if counter.invalid { 0x80 } else { 0 });
    Ok(out)
}

/// 解码 BCR（5 字节）。
///
/// 数据不足 5 字节时返回 [`Asdu102Error::TooShort`]。
pub fn decode_bcr(bytes: &[u8]) -> Result<BinaryCounter, Asdu102Error> {
    if bytes.len() < 5 {
        return Err(Asdu102Error::TooShort);
    }
    Ok(BinaryCounter {
        reading: u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
        sequence: bytes[4] & 0x1F,
        carry: bytes[4] & 0x20 != 0,
        adjusted: bytes[4] & 0x40 != 0,
        invalid: bytes[4] & 0x80 != 0,
    })
}

/// 编码 QCC（1 字节）。
///
/// 请求分组必须 0..=4，冻结方式必须 0..=3，否则返回
/// [`Asdu102Error::ValueOutOfRange`]。
pub fn encode_qcc(qcc: &Qcc) -> Result<u8, Asdu102Error> {
    if qcc.request > 0x04 || qcc.freeze > 0x03 {
        return Err(Asdu102Error::ValueOutOfRange);
    }
    Ok((qcc.request & 0x3F) | ((qcc.freeze & 0x03) << 6))
}

/// 解码 QCC（1 字节）。
pub fn decode_qcc(byte: u8) -> Qcc {
    Qcc {
        request: byte & 0x3F,
        freeze: (byte >> 6) & 0x03,
    }
}

/// 编码 CP24Time2a（3 字节）。
///
/// 毫秒必须 0..=59999，分钟必须 0..=59，否则返回
/// [`Asdu102Error::ValueOutOfRange`]。
pub fn encode_cp24time2a(time: &Cp24Time2a) -> Result<[u8; 3], Asdu102Error> {
    if time.milliseconds > 59999 || time.minute > 59 {
        return Err(Asdu102Error::ValueOutOfRange);
    }
    Ok([
        (time.milliseconds & 0xFF) as u8,
        ((time.milliseconds >> 8) & 0xFF) as u8,
        (time.minute & 0x3F) | if time.invalid { 0x80 } else { 0 },
    ])
}

/// 解码 CP24Time2a（3 字节）。
pub fn decode_cp24time2a(bytes: &[u8; 3]) -> Cp24Time2a {
    Cp24Time2a {
        milliseconds: u16::from_le_bytes([bytes[0], bytes[1]]),
        minute: bytes[2] & 0x3F,
        invalid: bytes[2] & 0x80 != 0,
    }
}

/// 编码 CP56Time2a（7 字节）。
///
/// 布局与 101 相同：
/// - b0..b1：毫秒，低字节在前；
/// - b2：IV(bit7) | 分钟(bit0-5)；
/// - b3：SU(bit7) | 小时(bit0-4)；
/// - b4：日(bit0-4)，高 3 位为 0；
/// - b5：月(bit0-3)；
/// - b6：年减去 2000，取低 7 位。
///
/// 时间字段必须在结构体注释给出的范围内，否则返回
/// [`Asdu102Error::ValueOutOfRange`]。
pub fn encode_cp56time2a(time: &Cp56Time2a) -> Result<[u8; 7], Asdu102Error> {
    if time.milliseconds > 59999
        || time.minute > 59
        || time.hour > 23
        || time.day == 0
        || time.day > 31
        || time.month == 0
        || time.month > 12
    {
        return Err(Asdu102Error::ValueOutOfRange);
    }
    Ok([
        (time.milliseconds & 0xFF) as u8,
        ((time.milliseconds >> 8) & 0xFF) as u8,
        (time.minute & 0x3F) | if time.invalid { 0x80 } else { 0 },
        (time.hour & 0x1F) | if time.summer_time { 0x80 } else { 0 },
        time.day & 0x1F,
        time.month & 0x0F,
        ((time.year.wrapping_sub(2000)) as u8) & 0x7F,
    ])
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

/// 编码一个信息对象的元素部分（不含 IOA）。
fn encode_element(
    out: &mut Vec<u8>,
    type_id: u8,
    object: &InformationObject102,
) -> Result<(), Asdu102Error> {
    match object {
        InformationObject102::Counter { counter, .. } => {
            if type_id != M_IT_NA_1 {
                return Err(Asdu102Error::TypeMismatch);
            }
            out.extend_from_slice(&encode_bcr(counter)?);
        }
        InformationObject102::CounterCp24 { counter, time, .. } => {
            if type_id != M_IT_TA_1 {
                return Err(Asdu102Error::TypeMismatch);
            }
            out.extend_from_slice(&encode_bcr(counter)?);
            out.extend_from_slice(&encode_cp24time2a(time)?);
        }
        InformationObject102::CounterCp56 { counter, time, .. } => {
            if type_id != M_IT_TC_1 {
                return Err(Asdu102Error::TypeMismatch);
            }
            out.extend_from_slice(&encode_bcr(counter)?);
            out.extend_from_slice(&encode_cp56time2a(time)?);
        }
        InformationObject102::CounterInterrogation { qcc, .. } => {
            if type_id != C_IT_NA_1 {
                return Err(Asdu102Error::TypeMismatch);
            }
            out.push(encode_qcc(qcc)?);
        }
        InformationObject102::CounterInterrogationCp24 { qcc, time, .. } => {
            if type_id != C_IT_TA_1 {
                return Err(Asdu102Error::TypeMismatch);
            }
            out.push(encode_qcc(qcc)?);
            out.extend_from_slice(&encode_cp24time2a(time)?);
        }
        InformationObject102::CounterInterrogationCp56 { qcc, time, .. } => {
            if type_id != C_IT_TC_1 {
                return Err(Asdu102Error::TypeMismatch);
            }
            out.push(encode_qcc(qcc)?);
            out.extend_from_slice(&encode_cp56time2a(time)?);
        }
    }
    Ok(())
}

/// 解码一个信息对象的元素部分（不含 IOA），返回对象与消费的字节数。
fn decode_element(
    type_id: u8,
    ioa: u32,
    bytes: &[u8],
) -> Result<(InformationObject102, usize), Asdu102Error> {
    match type_id {
        M_IT_NA_1 => {
            let counter = decode_bcr(bytes)?;
            Ok((InformationObject102::Counter { ioa, counter }, 5))
        }
        M_IT_TA_1 => {
            if bytes.len() < 8 {
                return Err(Asdu102Error::TooShort);
            }
            let counter = decode_bcr(&bytes[0..5])?;
            let time = decode_cp24time2a(&[bytes[5], bytes[6], bytes[7]]);
            Ok((InformationObject102::CounterCp24 { ioa, counter, time }, 8))
        }
        M_IT_TC_1 => {
            if bytes.len() < 12 {
                return Err(Asdu102Error::TooShort);
            }
            let counter = decode_bcr(&bytes[0..5])?;
            let time = decode_cp56time2a(&[
                bytes[5], bytes[6], bytes[7], bytes[8], bytes[9], bytes[10], bytes[11],
            ]);
            Ok((InformationObject102::CounterCp56 { ioa, counter, time }, 12))
        }
        C_IT_NA_1 => {
            if bytes.is_empty() {
                return Err(Asdu102Error::TooShort);
            }
            Ok((
                InformationObject102::CounterInterrogation {
                    ioa,
                    qcc: decode_qcc(bytes[0]),
                },
                1,
            ))
        }
        C_IT_TA_1 => {
            if bytes.len() < 4 {
                return Err(Asdu102Error::TooShort);
            }
            let qcc = decode_qcc(bytes[0]);
            let time = decode_cp24time2a(&[bytes[1], bytes[2], bytes[3]]);
            Ok((
                InformationObject102::CounterInterrogationCp24 { ioa, qcc, time },
                4,
            ))
        }
        C_IT_TC_1 => {
            if bytes.len() < 8 {
                return Err(Asdu102Error::TooShort);
            }
            let qcc = decode_qcc(bytes[0]);
            let time = decode_cp56time2a(&[
                bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
            ]);
            Ok((
                InformationObject102::CounterInterrogationCp56 { ioa, qcc, time },
                8,
            ))
        }
        _ => Err(Asdu102Error::UnsupportedType(type_id)),
    }
}

/// 编码 ASDU。
///
/// 信息对象数目必须大于 0 且不超过 127；传送原因必须 0..=63；地址不能超过
/// 地址长度允许的最大值；各元素字段必须在结构体注释给出的范围内。越界返回
/// [`Asdu102Error::ValueOutOfRange`]，类型标识不支持时返回
/// [`Asdu102Error::UnsupportedType`]，信息对象与类型标识不匹配时返回
/// [`Asdu102Error::TypeMismatch`]。
pub fn encode_asdu102(asdu: &Asdu102) -> Result<Vec<u8>, Asdu102Error> {
    let ca_len = validate_ca_len(asdu.address.ca_len)?;
    let ioa_len = validate_ioa_len(asdu.address.ioa_len)?;
    let elem_size =
        element_size(asdu.type_id).ok_or(Asdu102Error::UnsupportedType(asdu.type_id))?;

    let count = asdu.objects.len();
    if count == 0 {
        return Err(Asdu102Error::ObjectCountMismatch);
    }
    if count > 0x7F {
        return Err(Asdu102Error::ValueOutOfRange);
    }
    if asdu.cause > 0x3F {
        return Err(Asdu102Error::ValueOutOfRange);
    }
    if asdu.address.common_address as u32 > address_limit(ca_len) {
        return Err(Asdu102Error::ValueOutOfRange);
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
            return Err(Asdu102Error::ValueOutOfRange);
        }
        let last = base
            .checked_add(count as u32 - 1)
            .ok_or(Asdu102Error::ValueOutOfRange)?;
        if last > ioa_limit {
            return Err(Asdu102Error::ValueOutOfRange);
        }
        push_address(&mut out, base, ioa_len);
        for object in &asdu.objects {
            encode_element(&mut out, asdu.type_id, object)?;
        }
    } else {
        for object in &asdu.objects {
            let ioa = object.ioa();
            if ioa > ioa_limit {
                return Err(Asdu102Error::ValueOutOfRange);
            }
            push_address(&mut out, ioa, ioa_len);
            encode_element(&mut out, asdu.type_id, object)?;
        }
    }
    Ok(out)
}

/// 解码 ASDU，返回 ASDU 与已消费的字节数。
///
/// `bytes` 可以包含 ASDU 之后的额外数据，已消费字节数表示 ASDU 结束位置。
/// 缓冲区短于固定头部时返回 [`Asdu102Error::TooShort`]；头部可解析但声明的
/// 信息对象放不进缓冲区时返回 [`Asdu102Error::LengthMismatch`]；类型标识不
/// 支持时返回 [`Asdu102Error::UnsupportedType`]。
pub fn decode_asdu102(
    bytes: &[u8],
    address: StationAddress,
) -> Result<(Asdu102, usize), Asdu102Error> {
    let ca_len = validate_ca_len(address.ca_len)?;
    let ioa_len = validate_ioa_len(address.ioa_len)?;
    let header = 4 + ca_len;
    if bytes.len() < header {
        return Err(Asdu102Error::TooShort);
    }

    let type_id = bytes[0];
    let vsq = bytes[1];
    let sq = vsq & 0x80 != 0;
    let count = (vsq & 0x7F) as usize;
    let elem_size = element_size(type_id).ok_or(Asdu102Error::UnsupportedType(type_id))?;
    if count == 0 {
        return Err(Asdu102Error::ObjectCountMismatch);
    }

    let cause_byte = bytes[2];
    let test = cause_byte & 0x80 != 0;
    let negative = cause_byte & 0x40 != 0;
    let cause = cause_byte & 0x3F;
    let originator = bytes[3];
    let common_address = read_address(&bytes[4..4 + ca_len]) as u16;
    let decoded_address = StationAddress {
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
        return Err(Asdu102Error::LengthMismatch {
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
                .ok_or(Asdu102Error::ValueOutOfRange)?;
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
        Asdu102 {
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
