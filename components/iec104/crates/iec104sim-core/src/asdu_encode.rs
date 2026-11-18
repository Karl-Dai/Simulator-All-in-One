//! ASDU 编码辅助：时标 (CP56Time2a / CP24Time2a)。
//!
//! 这里集中放置不依赖运行时状态的纯函数,以便:
//! - 在 `slave.rs` 中替换硬编码的"只发无时标版本"逻辑;
//! - 用单元测试在不启动 Tokio 任务的情况下验证字节序。
//!
//! 后续步骤会在本模块继续加入 `encode_point_frame_ex` 与
//! `encode_points_grouped`,目前先完成时间编码部分。

use chrono::{DateTime, Datelike, Timelike, Utc};

/// 编码 CP56Time2a (7 字节):
/// ```text
/// b0..b1 : ms 低/高字节, ms = sec*1000 + millis (little-endian)
/// b2     : (IV<<7) | (RES1<<6) | minute(0..59)
/// b3     : (SU<<7) | (RES2<<5..6) | hour(0..23)
/// b4     : (DOW<<5) | day_of_month(1..31)
/// b5     : month(1..12)            (高 4 位保留)
/// b6     : (year - 2000) & 0x7F
/// ```
/// `iv = true` 表示无效时间戳。SU(夏令时) 统一为 false。
/// DOW 取 ISO 8601 周一=1..周日=7,与 IEC 60870-5-4 定义一致。
pub fn encode_cp56time2a(t: DateTime<Utc>, iv: bool) -> [u8; 7] {
    let ms_total = (t.second() as u16) * 1000 + (t.nanosecond() / 1_000_000) as u16;
    let minute = (t.minute() as u8) & 0x3F | if iv { 0x80 } else { 0 };
    let hour = (t.hour() as u8) & 0x1F; // SU=0
    let dow = t.weekday().number_from_monday() as u8; // 1..=7
    let day = t.day() as u8 & 0x1F;
    let month = t.month() as u8 & 0x0F;
    let year = ((t.year() - 2000) as i32).rem_euclid(128) as u8 & 0x7F;
    [
        (ms_total & 0xFF) as u8,
        ((ms_total >> 8) & 0xFF) as u8,
        minute,
        hour,
        (dow << 5) | day,
        month,
        year,
    ]
}

/// 编码 CP24Time2a (3 字节):
/// ```text
/// b0..b1 : ms 低/高字节
/// b2     : (IV<<7) | minute(0..59)
/// ```
/// 与 CP56Time2a 共用同一时间源,只截取毫秒和分钟。
pub fn encode_cp24time2a(t: DateTime<Utc>, iv: bool) -> [u8; 3] {
    let ms_total = (t.second() as u16) * 1000 + (t.nanosecond() / 1_000_000) as u16;
    let minute = (t.minute() as u8) & 0x3F | if iv { 0x80 } else { 0 };
    [
        (ms_total & 0xFF) as u8,
        ((ms_total >> 8) & 0xFF) as u8,
        minute,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn cp56time2a_known_instant() {
        // 2026-01-02 (周五) 03:04:05.123 UTC
        let t = Utc.with_ymd_and_hms(2026, 1, 2, 3, 4, 5).unwrap()
            + chrono::Duration::milliseconds(123);
        let bytes = encode_cp56time2a(t, false);
        let ms = 5_u16 * 1000 + 123;
        assert_eq!(bytes[0], (ms & 0xFF) as u8);
        assert_eq!(bytes[1], (ms >> 8) as u8);
        assert_eq!(bytes[2], 4); // minute, IV=0
        assert_eq!(bytes[3], 3); // hour, SU=0
        // 2026-01-02 是周五,ISO 周一=1..周五=5; day=2
        assert_eq!(bytes[4], (5 << 5) | 2);
        assert_eq!(bytes[5], 1); // month
        assert_eq!(bytes[6], 26); // year offset
    }

    #[test]
    fn cp56time2a_invalid_flag_sets_minute_msb() {
        let t = Utc.with_ymd_and_hms(2026, 1, 2, 3, 4, 5).unwrap();
        let bytes = encode_cp56time2a(t, true);
        assert_eq!(bytes[2] & 0x80, 0x80, "IV 位应被置 1");
        assert_eq!(bytes[2] & 0x3F, 4, "minute 低 6 位保留");
    }

    #[test]
    fn cp24time2a_known_instant() {
        let t = Utc.with_ymd_and_hms(2026, 1, 2, 3, 4, 5).unwrap()
            + chrono::Duration::milliseconds(789);
        let bytes = encode_cp24time2a(t, false);
        let ms = 5_u16 * 1000 + 789;
        assert_eq!(bytes[0], (ms & 0xFF) as u8);
        assert_eq!(bytes[1], (ms >> 8) as u8);
        assert_eq!(bytes[2], 4);
    }

    #[test]
    fn cp56time2a_year_wraps_modulo_128() {
        // 2099 → year offset 99 (在 0..128 内,直接截断)
        let t = Utc.with_ymd_and_hms(2099, 12, 31, 23, 59, 59).unwrap();
        let bytes = encode_cp56time2a(t, false);
        assert_eq!(bytes[6], 99);
    }
}
