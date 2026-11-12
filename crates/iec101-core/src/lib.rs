//! IEC 60870-5-101 实现。
//!
//! 模块分工：
//! - `ft12`：FT1.2 链路帧编解码（单字符、固定帧、可变帧）。
//! - `link`：非平衡链路状态机。
//! - `asdu`：101 应用层 ASDU 编解码。
//! - `session`：主站与子站的业务会话。
//! - `transport`：TCP 与串口传输。

pub mod asdu;
pub mod ft12;
pub mod link;
pub mod session;
pub mod transport;
