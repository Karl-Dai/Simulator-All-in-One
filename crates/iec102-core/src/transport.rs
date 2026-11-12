//! TCP 与串口传输层。
//!
//! [`FrameConnection`] 把 TCP 流和串口流统一成同一个异步读写接口，内部维护
//! 接收缓冲区，负责 `FT1.2` 帧的拆包、粘包处理与错误恢复。上层只需调用
//! [`FrameConnection::read_frame`] 与 [`FrameConnection::write_frame`]，不必关心
//! 底层是网络还是串口。

use std::io;
use std::sync::atomic::{AtomicU64, Ordering};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_serial::SerialPortBuilderExt;

use crate::ft12::{encode_frame, parse_frame, Frame, LinkConfig, ParseError};

/// 传输统计。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TransportStats {
    /// 成功解析的帧数。
    pub frames_read: u64,
    /// 成功编码并写出的帧数。
    pub frames_written: u64,
    /// 从传输层读到的原始字节数。
    pub bytes_read: u64,
    /// 写到传输层的原始字节数。
    pub bytes_written: u64,
    /// 校验和错误的次数。
    pub checksum_errors: u64,
    /// 其他解析错误的次数。
    pub parse_errors: u64,
}

/// 串口校验方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Parity {
    /// 无校验。
    None,
    /// 奇校验。
    Odd,
    /// 偶校验。
    Even,
}

/// 串口参数。默认 9600、8 数据位、偶校验、1 停止位。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SerialSettings {
    /// 串口设备路径，例如 `/dev/ttyUSB0`。
    pub port: String,
    /// 波特率。
    pub baud_rate: u32,
    /// 数据位，取 5 到 8。
    pub data_bits: u8,
    /// 校验方式。
    pub parity: Parity,
    /// 停止位，取 1 或 2。
    pub stop_bits: u8,
}

impl Default for SerialSettings {
    fn default() -> Self {
        Self {
            port: String::new(),
            baud_rate: 9600,
            data_bits: 8,
            parity: Parity::Even,
            stop_bits: 1,
        }
    }
}

/// 内部统计，使用原子计数以便在 `&self` 上读取。
#[derive(Debug, Default)]
struct AtomicStats {
    frames_read: AtomicU64,
    frames_written: AtomicU64,
    bytes_read: AtomicU64,
    bytes_written: AtomicU64,
    checksum_errors: AtomicU64,
    parse_errors: AtomicU64,
}

impl AtomicStats {
    fn snapshot(&self) -> TransportStats {
        TransportStats {
            frames_read: self.frames_read.load(Ordering::Relaxed),
            frames_written: self.frames_written.load(Ordering::Relaxed),
            bytes_read: self.bytes_read.load(Ordering::Relaxed),
            bytes_written: self.bytes_written.load(Ordering::Relaxed),
            checksum_errors: self.checksum_errors.load(Ordering::Relaxed),
            parse_errors: self.parse_errors.load(Ordering::Relaxed),
        }
    }
}

/// 底层传输对象，屏蔽 TCP 与串口的差异。
#[derive(Debug)]
enum Transport {
    Tcp(TcpStream),
    Serial(tokio_serial::SerialStream),
}

impl Transport {
    async fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Transport::Tcp(stream) => stream.read(buf).await,
            Transport::Serial(stream) => stream.read(buf).await,
        }
    }

    async fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        match self {
            Transport::Tcp(stream) => stream.write_all(buf).await,
            Transport::Serial(stream) => stream.write_all(buf).await,
        }
    }
}

/// 一条链路连接，屏蔽 TCP 与串口差异。
#[derive(Debug)]
pub struct FrameConnection {
    transport: Transport,
    /// 接收缓冲区，存放尚未组成完整帧的字节。
    buffer: Vec<u8>,
    stats: AtomicStats,
    peer: String,
}

impl FrameConnection {
    /// 用已有的传输对象构造连接。
    fn new(transport: Transport, peer: String) -> Self {
        Self {
            transport,
            buffer: Vec::new(),
            stats: AtomicStats::default(),
            peer,
        }
    }

    /// 主动连接 TCP 对端。
    pub async fn connect_tcp(address: &str) -> io::Result<Self> {
        let stream = TcpStream::connect(address).await?;
        let peer = match stream.peer_addr() {
            Ok(addr) => format!("tcp {}", addr),
            Err(_) => format!("tcp {}", address),
        };
        Ok(Self::new(Transport::Tcp(stream), peer))
    }

    /// 接受一个 TCP 连接。
    pub async fn accept_tcp(listener: &TcpListener) -> io::Result<Self> {
        let (stream, addr) = listener.accept().await?;
        Ok(Self::new(Transport::Tcp(stream), format!("tcp {}", addr)))
    }

    /// 打开串口。
    pub async fn open_serial(settings: &SerialSettings) -> io::Result<Self> {
        let data_bits = match settings.data_bits {
            5 => tokio_serial::DataBits::Five,
            6 => tokio_serial::DataBits::Six,
            7 => tokio_serial::DataBits::Seven,
            8 => tokio_serial::DataBits::Eight,
            other => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("数据位只能是 5 到 8，当前为 {}", other),
                ))
            }
        };
        let parity = match settings.parity {
            Parity::None => tokio_serial::Parity::None,
            Parity::Odd => tokio_serial::Parity::Odd,
            Parity::Even => tokio_serial::Parity::Even,
        };
        let stop_bits = match settings.stop_bits {
            1 => tokio_serial::StopBits::One,
            2 => tokio_serial::StopBits::Two,
            other => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("停止位只能是 1 或 2，当前为 {}", other),
                ))
            }
        };

        let builder = tokio_serial::new(settings.port.as_str(), settings.baud_rate)
            .data_bits(data_bits)
            .parity(parity)
            .stop_bits(stop_bits);
        let stream = builder
            .open_native_async()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        Ok(Self::new(
            Transport::Serial(stream),
            format!("serial {}", settings.port),
        ))
    }

    /// 读取一帧。内部维持缓冲区，处理粘包与半包；遇到非法字节时丢弃并计入
    /// `parse_errors` 或 `checksum_errors`。
    pub async fn read_frame(&mut self, config: &LinkConfig) -> io::Result<Frame> {
        loop {
            match parse_frame(&self.buffer, config) {
                Ok((frame, used)) => {
                    self.buffer.drain(..used);
                    self.stats.frames_read.fetch_add(1, Ordering::Relaxed);
                    return Ok(frame);
                }
                Err(ParseError::Incomplete) => {
                    let mut chunk = [0u8; 4096];
                    let read = self.transport.read(&mut chunk).await?;
                    if read == 0 {
                        return Err(io::Error::new(
                            io::ErrorKind::UnexpectedEof,
                            "对端已关闭连接",
                        ));
                    }
                    self.buffer.extend_from_slice(&chunk[..read]);
                    self.stats.bytes_read.fetch_add(read as u64, Ordering::Relaxed);
                }
                Err(ParseError::InvalidAddressLength) => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "链路地址域长度只能是 1 或 2",
                    ));
                }
                Err(err) => {
                    // 丢弃一个字节后继续找下一帧。
                    self.buffer.drain(..1);
                    if matches!(err, ParseError::BadChecksum { .. }) {
                        self.stats.checksum_errors.fetch_add(1, Ordering::Relaxed);
                    } else {
                        self.stats.parse_errors.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        }
    }

    /// 编码并写出一帧。
    pub async fn write_frame(&mut self, frame: &Frame, config: &LinkConfig) -> io::Result<()> {
        let bytes = encode_frame(frame, config)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, format!("帧编码失败: {e:?}")))?;
        self.write_bytes(&bytes).await?;
        self.stats.frames_written.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    /// 直接写原始字节，供需要精确控制报文的测试使用。
    pub async fn write_bytes(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.transport.write_all(bytes).await?;
        self.stats.bytes_written.fetch_add(bytes.len() as u64, Ordering::Relaxed);
        Ok(())
    }

    /// 返回当前统计。
    pub fn stats(&self) -> TransportStats {
        self.stats.snapshot()
    }

    /// 对端地址描述，用于界面展示，例如 `tcp 127.0.0.1:2402`。
    pub fn peer(&self) -> String {
        self.peer.clone()
    }
}
