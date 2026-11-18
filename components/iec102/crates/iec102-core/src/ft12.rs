//! FT1.2 链路帧编解码。
//!
//! 支持三种帧：
//!
//! - 单字符帧：`0xE5`，表示肯定认可。
//! - 固定帧：`0x10 | control | address | checksum | 0x16`。
//! - 可变帧：`0x68 | L | L | L | L | control | address | asdu | checksum | 0x16`。
//!
//! 可变帧的 `L` 等于控制域长度、地址域长度与 ASDU 长度之和，取值 1..=255。第 2
//! 到第 5 字节冗余发送同一个 `L`，接收端以第 2 字节为准。校验和为控制域首字节
//! 到 ASDU 末字节的算术和，取低 8 位。

/// 固定帧启动字符。
const START_FIXED: u8 = 0x10;
/// 可变帧启动字符。
const START_VARIABLE: u8 = 0x68;
/// 帧结束字符。
const END: u8 = 0x16;
/// 单字符肯定认可。
const ACK: u8 = 0xE5;
/// 功能码掩码。
const FUNCTION_MASK: u8 = 0x0F;

/// 链路配置。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinkConfig {
    /// 地址域长度，取 1 或 2。
    pub address_len: u8,
}

impl LinkConfig {
    /// 创建链路配置。
    ///
    /// `address_len` 应为 1 或 2。传入其他值不会在此处报错，而是在解析时返回
    /// [`ParseError::InvalidAddressLength`]。
    pub const fn new(address_len: u8) -> Self {
        Self { address_len }
    }

    /// 返回地址域字节数。
    pub fn address_size(&self) -> usize {
        self.address_len as usize
    }

    /// 校验地址域长度，返回合法长度或错误。
    fn valid_address_size(&self) -> Result<usize, ParseError> {
        match self.address_len {
            1 | 2 => Ok(self.address_len as usize),
            _ => Err(ParseError::InvalidAddressLength),
        }
    }
}

/// FT1.2 链路帧。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Frame {
    /// 单字符肯定认可。
    Ack,
    /// 固定帧。
    Fixed {
        /// 控制域。
        control: u8,
        /// 链路地址。
        address: u16,
    },
    /// 可变帧。
    Variable {
        /// 控制域。
        control: u8,
        /// 链路地址。
        address: u16,
        /// ASDU 字节。
        asdu: Vec<u8>,
    },
}

/// 帧解析错误。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseError {
    /// 数据不完整，需要继续接收。
    Incomplete,
    /// 启动字符错误。
    BadStartByte,
    /// 结束字符错误。
    BadEndByte,
    /// 校验和错误。`expected` 为计算值，`actual` 为帧中的值。
    BadChecksum {
        /// 计算得到的校验和。
        expected: u8,
        /// 帧中的校验和。
        actual: u8,
    },
    /// 长度字段与实际内容不一致。
    BadLength,
    /// 地址域长度非法。
    InvalidAddressLength,
}

/// 帧编码错误。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodeError {
    /// 地址域长度不是 1 或 2。
    InvalidAddressLength,
    /// 帧长度超过单字节长度域能表示的上限 255。
    FrameTooLong,
}

/// 计算校验和：`bytes` 的算术和，取低 8 位。
pub fn checksum(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0u8, |sum, &b| sum.wrapping_add(b))
}

/// 解析一帧。
///
/// 成功时返回帧和已消费的字节数。数据不完整时返回 [`ParseError::Incomplete`]。
pub fn parse_frame(buf: &[u8], config: &LinkConfig) -> Result<(Frame, usize), ParseError> {
    if buf.is_empty() {
        return Err(ParseError::Incomplete);
    }
    match buf[0] {
        ACK => Ok((Frame::Ack, 1)),
        START_FIXED => {
            let addr_len = config.valid_address_size()?;
            parse_fixed(buf, addr_len)
        }
        START_VARIABLE => {
            let addr_len = config.valid_address_size()?;
            parse_variable(buf, addr_len)
        }
        _ => Err(ParseError::BadStartByte),
    }
}

/// 编码一帧。
///
/// 单字符帧不受地址域长度影响，永远编码成功。固定帧与可变帧在 `config.address_len`
/// 不是 1 或 2 时返回 [`EncodeError::InvalidAddressLength`]。可变帧的长度超过 255
/// 时返回 [`EncodeError::FrameTooLong`]。
pub fn encode_frame(frame: &Frame, config: &LinkConfig) -> Result<Vec<u8>, EncodeError> {
    match frame {
        Frame::Ack => Ok(vec![ACK]),
        Frame::Fixed { control, address } => {
            let addr_len = encode_address_size(config)?;
            let mut out = Vec::with_capacity(4 + addr_len);
            out.push(START_FIXED);
            out.push(*control);
            push_address(&mut out, *address, addr_len);
            let sum = checksum(&out[1..]);
            out.push(sum);
            out.push(END);
            Ok(out)
        }
        Frame::Variable {
            control,
            address,
            asdu,
        } => {
            let addr_len = encode_address_size(config)?;
            let length = 1 + addr_len + asdu.len();
            if length > u8::MAX as usize {
                return Err(EncodeError::FrameTooLong);
            }
            let length = length as u8;
            let mut out = Vec::with_capacity(7 + length as usize);
            out.push(START_VARIABLE);
            out.extend_from_slice(&[length; 4]);
            out.push(*control);
            push_address(&mut out, *address, addr_len);
            out.extend_from_slice(asdu);
            let sum = checksum(&out[5..]);
            out.push(sum);
            out.push(END);
            Ok(out)
        }
    }
}

/// 校验编码用的地址域长度，返回合法长度或错误。
fn encode_address_size(config: &LinkConfig) -> Result<usize, EncodeError> {
    match config.address_len {
        1 | 2 => Ok(config.address_len as usize),
        _ => Err(EncodeError::InvalidAddressLength),
    }
}

/// 解析固定帧。总长度为 `4 + addr_len`。
fn parse_fixed(buf: &[u8], addr_len: usize) -> Result<(Frame, usize), ParseError> {
    let total = 4 + addr_len;
    if buf.len() < total {
        return Err(ParseError::Incomplete);
    }
    let control = buf[1];
    let address = read_address(&buf[2..2 + addr_len]);
    let actual = buf[2 + addr_len];
    let expected = checksum(&buf[1..2 + addr_len]);
    if buf[3 + addr_len] != END {
        return Err(ParseError::BadEndByte);
    }
    if actual != expected {
        return Err(ParseError::BadChecksum { expected, actual });
    }
    Ok((Frame::Fixed { control, address }, total))
}

/// 解析可变帧。`L` 为控制域、地址域与 ASDU 长度之和，总长度为 `7 + L`。
fn parse_variable(buf: &[u8], addr_len: usize) -> Result<(Frame, usize), ParseError> {
    if buf.len() < 5 {
        return Err(ParseError::Incomplete);
    }
    let length = buf[1] as usize;
    // 第 2 到第 5 字节必须是同一个 L。以第 2 字节为准，其余不一致直接报错。
    if buf[2] != buf[1] || buf[3] != buf[1] || buf[4] != buf[1] {
        return Err(ParseError::BadLength);
    }
    // 控制域占 1 字节，地址域占 addr_len 字节，ASDU 长度不能为负。
    if length < 1 + addr_len {
        return Err(ParseError::BadLength);
    }
    let total = 7 + length;
    if buf.len() < total {
        return Err(ParseError::Incomplete);
    }
    let control = buf[5];
    let address = read_address(&buf[6..6 + addr_len]);
    let asdu = buf[6 + addr_len..5 + length].to_vec();
    let actual = buf[5 + length];
    let expected = checksum(&buf[5..5 + length]);
    if buf[6 + length] != END {
        return Err(ParseError::BadEndByte);
    }
    if actual != expected {
        return Err(ParseError::BadChecksum { expected, actual });
    }
    Ok((Frame::Variable {
        control,
        address,
        asdu,
    }, total))
}

/// 读取地址域，低字节在前。`bytes` 长度为 1 或 2。
fn read_address(bytes: &[u8]) -> u16 {
    let mut address = bytes[0] as u16;
    if bytes.len() == 2 {
        address |= (bytes[1] as u16) << 8;
    }
    address
}

/// 写入地址域，低字节在前。`addr_len` 为 1 或 2。
fn push_address(out: &mut Vec<u8>, address: u16, addr_len: usize) {
    out.push((address & 0xFF) as u8);
    if addr_len == 2 {
        out.push(((address >> 8) & 0xFF) as u8);
    }
}

/// 控制域位解析与编码。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlField {
    /// 传输方向，bit7。
    pub dir: bool,
    /// 启动报文位，bit6。
    pub prm: bool,
    /// 帧计数位，bit5，仅 `prm` 为 1 时有意义。
    pub fcb: bool,
    /// 帧计数有效位，bit4，仅 `prm` 为 1 时有意义。
    pub fcv: bool,
    /// 访问位，bit4，仅 `prm` 为 0 时有意义。
    pub acd: bool,
    /// 数据流控制位，bit5，仅 `prm` 为 0 时有意义。
    pub dfc: bool,
    /// 功能码，bit0-3。
    pub function: u8,
}

impl ControlField {
    /// 从控制域字节解析。
    pub fn parse(control: u8) -> Self {
        Self {
            dir: control & 0x80 != 0,
            prm: control & 0x40 != 0,
            fcb: control & 0x20 != 0,
            fcv: control & 0x10 != 0,
            acd: control & 0x10 != 0,
            dfc: control & 0x20 != 0,
            function: control & FUNCTION_MASK,
        }
    }

    /// 编码成控制域字节。
    ///
    /// `prm` 为 1 时使用 `fcb` 与 `fcv`，否则使用 `acd` 与 `dfc`。
    pub fn encode(&self) -> u8 {
        let mut out = self.function & FUNCTION_MASK;
        if self.dir {
            out |= 0x80;
        }
        if self.prm {
            out |= 0x40;
            if self.fcb {
                out |= 0x20;
            }
            if self.fcv {
                out |= 0x10;
            }
        } else {
            if self.acd {
                out |= 0x10;
            }
            if self.dfc {
                out |= 0x20;
            }
        }
        out
    }
}
