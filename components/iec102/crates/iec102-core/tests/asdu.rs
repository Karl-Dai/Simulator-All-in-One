//! 102 累计量 ASDU 编解码测试。

use iec102_core::asdu::{
    decode_asdu102, decode_bcr, decode_cp24time2a, decode_cp56time2a, decode_qcc, encode_asdu102,
    encode_bcr, encode_cp24time2a, encode_cp56time2a, encode_qcc, Asdu102, Asdu102Error,
    BinaryCounter, Cp24Time2a, Cp56Time2a, InformationObject102, Qcc, StationAddress,
};

/// 默认地址配置：CA=1 字节、IOA=2 字节。
fn default_address() -> StationAddress {
    StationAddress {
        common_address: 1,
        ca_len: 1,
        ioa_len: 2,
    }
}

/// 构造 ASDU，默认 SQ=0、原因 3（突发）、无试验与否定确认。
fn make_asdu(type_id: u8, objects: Vec<InformationObject102>) -> Asdu102 {
    Asdu102 {
        type_id,
        sq: false,
        cause: 3,
        negative: false,
        test: false,
        originator: 0,
        address: default_address(),
        objects,
    }
}

/// 构造 BCR。
fn bcr(reading: u32, sequence: u8, carry: bool, adjusted: bool, invalid: bool) -> BinaryCounter {
    BinaryCounter {
        reading,
        sequence,
        carry,
        adjusted,
        invalid,
    }
}

/// 构造 QCC。
fn qcc(request: u8, freeze: u8) -> Qcc {
    Qcc { request, freeze }
}

/// 类型 100 固定十六进制用例：读数 123456、顺序号 7、CY=1、CA=0、IV=0。
#[test]
fn type100_fixed_hex_bytes() {
    let asdu = Asdu102 {
        type_id: 100,
        sq: false,
        cause: 3,
        negative: false,
        test: false,
        originator: 0,
        address: default_address(),
        objects: vec![InformationObject102::Counter {
            ioa: 1,
            counter: bcr(123456, 7, true, false, false),
        }],
    };
    let bytes = encode_asdu102(&asdu).expect("编码应成功");
    assert_eq!(
        bytes,
        vec![
            0x64, // 类型标识 100
            0x01, // VSQ：SQ=0，信息对象数目 1
            0x03, // 传送原因 3
            0x00, // 源发地址
            0x01, // 公共地址 1
            0x01, 0x00, // IOA 1，低字节在前
            0x40, 0xE2, 0x01, 0x00, // 123456 = 0x0001E240，小端
            0x27, // SEQ=7 | CY=1<<5
        ]
    );
}

/// 类型 100 往返：读数 0 与 u32::MAX，顺序号 0 与 31。
#[test]
fn type100_roundtrip_boundaries() {
    let addr = default_address();
    let cases = [
        bcr(0, 0, false, false, false),
        bcr(u32::MAX, 31, true, true, true),
    ];
    for counter in cases {
        let asdu = make_asdu(
            100,
            vec![InformationObject102::Counter {
                ioa: 0x1234,
                counter,
            }],
        );
        let bytes = encode_asdu102(&asdu).expect("编码应成功");
        let (decoded, used) = decode_asdu102(&bytes, addr).expect("解码应成功");
        assert_eq!(used, bytes.len());
        assert_eq!(decoded.type_id, 100);
        assert_eq!(
            decoded.objects,
            vec![InformationObject102::Counter {
                ioa: 0x1234,
                counter
            }]
        );
    }
}

/// 类型 101 往返，核对 CP24Time2a 三字节。
#[test]
fn type101_roundtrip_and_cp24_bytes() {
    let time = Cp24Time2a {
        milliseconds: 12345,
        minute: 42,
        invalid: true,
    };
    // 12345 = 0x3039，低字节在前；bit7 为 IV。
    assert_eq!(
        encode_cp24time2a(&time).expect("编码应成功"),
        [0x39, 0x30, 0xAA]
    );
    assert_eq!(decode_cp24time2a(&[0x39, 0x30, 0xAA]), time);

    let counter = bcr(99, 3, false, true, false);
    let asdu = make_asdu(
        101,
        vec![InformationObject102::CounterCp24 {
            ioa: 0x0203,
            counter,
            time,
        }],
    );
    let bytes = encode_asdu102(&asdu).expect("编码应成功");
    let (decoded, used) = decode_asdu102(&bytes, default_address()).expect("解码应成功");
    assert_eq!(used, bytes.len());
    assert_eq!(
        decoded.objects,
        vec![InformationObject102::CounterCp24 {
            ioa: 0x0203,
            counter,
            time
        }]
    );
}

/// 类型 103 往返，核对 CP56Time2a 七字节。
#[test]
fn type103_roundtrip_and_cp56_bytes() {
    let time = Cp56Time2a {
        milliseconds: 12345,
        minute: 4,
        hour: 3,
        day: 2,
        month: 1,
        year: 2026,
        invalid: false,
        summer_time: false,
    };
    // 12345 = 0x3039；2026 - 2000 = 26 = 0x1A。
    assert_eq!(
        encode_cp56time2a(&time).expect("编码应成功"),
        [0x39, 0x30, 0x04, 0x03, 0x02, 0x01, 0x1A]
    );
    assert_eq!(
        decode_cp56time2a(&[0x39, 0x30, 0x04, 0x03, 0x02, 0x01, 0x1A]),
        time
    );

    let counter = bcr(u32::MAX, 0, false, false, true);
    let asdu = make_asdu(
        103,
        vec![InformationObject102::CounterCp56 {
            ioa: 0x0304,
            counter,
            time,
        }],
    );
    let bytes = encode_asdu102(&asdu).expect("编码应成功");
    let (decoded, used) = decode_asdu102(&bytes, default_address()).expect("解码应成功");
    assert_eq!(used, bytes.len());
    assert_eq!(
        decoded.objects,
        vec![InformationObject102::CounterCp56 {
            ioa: 0x0304,
            counter,
            time
        }]
    );
}

/// 类型 120 四种冻结方式往返，请求分组边界值 0 与 4。
#[test]
fn type120_freeze_modes_and_request_boundaries() {
    let addr = default_address();
    for freeze in 0..=3u8 {
        for request in [0u8, 1, 4] {
            let q = qcc(request, freeze);
            let expected = (request & 0x3F) | ((freeze & 0x03) << 6);
            assert_eq!(encode_qcc(&q).expect("编码应成功"), expected);
            assert_eq!(decode_qcc(expected), q);

            let asdu = make_asdu(
                120,
                vec![InformationObject102::CounterInterrogation {
                    ioa: 0x0506,
                    qcc: q,
                }],
            );
            let bytes = encode_asdu102(&asdu).expect("编码应成功");
            let (decoded, used) = decode_asdu102(&bytes, addr).expect("解码应成功");
            assert_eq!(used, bytes.len());
            assert_eq!(
                decoded.objects,
                vec![InformationObject102::CounterInterrogation {
                    ioa: 0x0506,
                    qcc: q
                }]
            );
        }
    }
}

/// 类型 121 与 123 往返。
#[test]
fn type121_and_123_roundtrip() {
    let addr = default_address();
    let q = qcc(2, 1);
    let time24 = Cp24Time2a {
        milliseconds: 59999,
        minute: 59,
        invalid: true,
    };
    let time56 = Cp56Time2a {
        milliseconds: 59999,
        minute: 59,
        hour: 23,
        day: 31,
        month: 12,
        year: 2127,
        invalid: true,
        summer_time: true,
    };

    let asdu121 = make_asdu(
        121,
        vec![InformationObject102::CounterInterrogationCp24 {
            ioa: 0x0708,
            qcc: q,
            time: time24,
        }],
    );
    let bytes = encode_asdu102(&asdu121).expect("编码应成功");
    let (decoded, used) = decode_asdu102(&bytes, addr).expect("解码应成功");
    assert_eq!(used, bytes.len());
    assert_eq!(decoded.objects, asdu121.objects);

    let asdu123 = make_asdu(
        123,
        vec![InformationObject102::CounterInterrogationCp56 {
            ioa: 0x090A,
            qcc: q,
            time: time56,
        }],
    );
    let bytes = encode_asdu102(&asdu123).expect("编码应成功");
    let (decoded, used) = decode_asdu102(&bytes, addr).expect("解码应成功");
    assert_eq!(used, bytes.len());
    assert_eq!(decoded.objects, asdu123.objects);
}

/// SQ=1 三个信息对象只写第一个地址，解码时地址依次加 1。
#[test]
fn sq1_writes_first_ioa_only_and_increments() {
    let addr = default_address();
    let objects: Vec<InformationObject102> = [10u32, 11, 12]
        .iter()
        .enumerate()
        .map(|(i, &ioa)| InformationObject102::Counter {
            ioa,
            counter: bcr(10 + i as u32, i as u8, false, false, false),
        })
        .collect();
    let asdu = Asdu102 {
        type_id: 100,
        sq: true,
        ..make_asdu(100, objects)
    };
    let bytes = encode_asdu102(&asdu).expect("编码应成功");
    // 头部 4 + CA 1 + IOA 2 = 7，之后只有 3 个 BCR，不再重复 IOA。
    assert_eq!(bytes.len(), 7 + 3 * 5);
    assert_eq!(&bytes[5..7], &[0x0A, 0x00]);

    let (decoded, used) = decode_asdu102(&bytes, addr).expect("解码应成功");
    assert_eq!(used, bytes.len());
    assert!(decoded.sq);
    let ioas: Vec<u32> = decoded
        .objects
        .iter()
        .map(InformationObject102::ioa)
        .collect();
    assert_eq!(ioas, vec![10, 11, 12]);
}

/// CA 2 字节、IOA 2 字节往返，低字节在前。
#[test]
fn two_byte_ca_and_ioa_roundtrip() {
    let addr = StationAddress {
        common_address: 0xBEEF,
        ca_len: 2,
        ioa_len: 2,
    };
    let asdu = Asdu102 {
        address: addr,
        ..make_asdu(
            100,
            vec![InformationObject102::Counter {
                ioa: 0x1234,
                counter: bcr(0xDEAD_BEEF, 15, true, false, true),
            }],
        )
    };
    let bytes = encode_asdu102(&asdu).expect("编码应成功");
    // 头部：类型、VSQ、原因、OA、CA 两字节（低字节在前）。
    assert_eq!(&bytes[4..6], &[0xEF, 0xBE]);
    assert_eq!(&bytes[6..8], &[0x34, 0x12]);
    let (decoded, used) = decode_asdu102(&bytes, addr).expect("解码应成功");
    assert_eq!(used, bytes.len());
    assert_eq!(decoded.address.common_address, 0xBEEF);
    assert_eq!(decoded.objects, asdu.objects);
}

/// 类型 102 解码返回 UnsupportedType(102)，编码也不支持。
#[test]
fn type102_is_unsupported() {
    // 类型 102 的固定头部 + 足够长的数据，但类型本身不支持。
    let bytes = [
        0x66, 0x01, 0x03, 0x00, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    let err = decode_asdu102(&bytes, default_address()).expect_err("类型 102 应报错");
    assert_eq!(err, Asdu102Error::UnsupportedType(102));

    let asdu = make_asdu(
        102,
        vec![InformationObject102::Counter {
            ioa: 1,
            counter: bcr(1, 0, false, false, false),
        }],
    );
    assert_eq!(
        encode_asdu102(&asdu),
        Err(Asdu102Error::UnsupportedType(102))
    );
}

/// 顺序号 32 与冻结方式 4 编码返回 ValueOutOfRange。
#[test]
fn value_range_checks() {
    assert_eq!(
        encode_bcr(&bcr(0, 32, false, false, false)),
        Err(Asdu102Error::ValueOutOfRange)
    );
    assert_eq!(
        encode_bcr(&bcr(0, 31, false, false, false))
            .expect("顺序号 31 合法")
            .len(),
        5
    );

    assert_eq!(encode_qcc(&qcc(0, 4)), Err(Asdu102Error::ValueOutOfRange));
    assert_eq!(encode_qcc(&qcc(5, 0)), Err(Asdu102Error::ValueOutOfRange));
    assert_eq!(encode_qcc(&qcc(4, 3)).expect("边界合法"), 0xC4);
}

/// 截断数据返回 TooShort 或 LengthMismatch。
#[test]
fn truncated_data_reports_error() {
    let addr = default_address();
    // 固定头部需要 5 字节，只给 2 字节。
    assert_eq!(
        decode_asdu102(&[0x64, 0x01], addr),
        Err(Asdu102Error::TooShort)
    );

    let asdu = make_asdu(
        100,
        vec![InformationObject102::Counter {
            ioa: 1,
            counter: bcr(1, 0, false, false, false),
        }],
    );
    let bytes = encode_asdu102(&asdu).expect("编码应成功");
    // 头部完整，但最后一个元素少 1 字节。
    assert!(matches!(
        decode_asdu102(&bytes[..bytes.len() - 1], addr),
        Err(Asdu102Error::LengthMismatch { .. })
    ));

    // decode_bcr 数据不足 5 字节。
    assert_eq!(decode_bcr(&[0x01, 0x02]), Err(Asdu102Error::TooShort));
}

/// 信息对象类型与类型标识不匹配时返回 TypeMismatch。
#[test]
fn type_mismatch_on_encode() {
    let asdu = make_asdu(
        100,
        vec![InformationObject102::CounterCp24 {
            ioa: 1,
            counter: bcr(1, 0, false, false, false),
            time: Cp24Time2a {
                milliseconds: 0,
                minute: 0,
                invalid: false,
            },
        }],
    );
    assert_eq!(encode_asdu102(&asdu), Err(Asdu102Error::TypeMismatch));
}

/// 信息对象数目为 0 时不合法。
#[test]
fn zero_object_count() {
    let asdu = make_asdu(100, vec![]);
    assert_eq!(
        encode_asdu102(&asdu),
        Err(Asdu102Error::ObjectCountMismatch)
    );

    let bytes = [0x64, 0x00, 0x03, 0x00, 0x01];
    assert_eq!(
        decode_asdu102(&bytes, default_address()),
        Err(Asdu102Error::ObjectCountMismatch)
    );
}

/// 地址长度非法时返回 UnsupportedAddressLength。
#[test]
fn unsupported_address_length() {
    let bad = StationAddress {
        common_address: 1,
        ca_len: 3,
        ioa_len: 2,
    };
    assert_eq!(
        decode_asdu102(&[0x64, 0x01, 0x03, 0x00, 0x01], bad),
        Err(Asdu102Error::UnsupportedAddressLength)
    );

    let asdu = Asdu102 {
        address: StationAddress {
            common_address: 1,
            ca_len: 0,
            ioa_len: 2,
        },
        ..make_asdu(
            100,
            vec![InformationObject102::Counter {
                ioa: 1,
                counter: bcr(1, 0, false, false, false),
            }],
        )
    };
    assert_eq!(
        encode_asdu102(&asdu),
        Err(Asdu102Error::UnsupportedAddressLength)
    );
}

/// 传送原因标志位与源发地址往返。
#[test]
fn cause_flags_and_originator_roundtrip() {
    let asdu = Asdu102 {
        test: true,
        negative: true,
        cause: 63,
        originator: 0x5A,
        ..make_asdu(
            100,
            vec![InformationObject102::Counter {
                ioa: 1,
                counter: bcr(1, 0, false, false, false),
            }],
        )
    };
    let bytes = encode_asdu102(&asdu).expect("编码应成功");
    assert_eq!(bytes[2], 0x80 | 0x40 | 63);
    assert_eq!(bytes[3], 0x5A);

    let (decoded, used) = decode_asdu102(&bytes, default_address()).expect("解码应成功");
    assert_eq!(used, bytes.len());
    assert!(decoded.test);
    assert!(decoded.negative);
    assert_eq!(decoded.cause, 63);
    assert_eq!(decoded.originator, 0x5A);
}

/// 解码带尾随数据时，返回值中的已消费字节数小于输入长度。
#[test]
fn decode_reports_consumed_bytes_with_trailing_data() {
    let asdu = make_asdu(
        100,
        vec![InformationObject102::Counter {
            ioa: 1,
            counter: bcr(7, 0, false, false, false),
        }],
    );
    let bytes = encode_asdu102(&asdu).expect("编码应成功");
    let mut padded = bytes.clone();
    padded.extend_from_slice(&[0xAA, 0xBB]);

    let (decoded, used) = decode_asdu102(&padded, default_address()).expect("解码应成功");
    assert_eq!(used, bytes.len());
    assert_eq!(decoded.objects, asdu.objects);
}

/// SQ=1 的累计量召唤也只用一个 IOA。
#[test]
fn sq1_counter_interrogation() {
    let addr = default_address();
    let objects: Vec<InformationObject102> = [0u8, 1, 2]
        .iter()
        .map(|&i| InformationObject102::CounterInterrogation {
            ioa: 20 + i as u32,
            qcc: qcc(i, i),
        })
        .collect();
    let asdu = Asdu102 {
        type_id: 120,
        sq: true,
        ..make_asdu(120, objects)
    };
    let bytes = encode_asdu102(&asdu).expect("编码应成功");
    // 头部 7 + 3 个 QCC。
    assert_eq!(bytes.len(), 7 + 3);
    assert_eq!(&bytes[5..7], &[0x14, 0x00]);

    let (decoded, used) = decode_asdu102(&bytes, addr).expect("解码应成功");
    assert_eq!(used, bytes.len());
    assert_eq!(decoded.objects.len(), 3);
    let ioas: Vec<u32> = decoded
        .objects
        .iter()
        .map(InformationObject102::ioa)
        .collect();
    assert_eq!(ioas, vec![20, 21, 22]);
}

/// 101/103 的 CP24Time2a 与 CP56Time2a 边界值往返。
#[test]
fn time_boundaries_roundtrip() {
    let cp24 = Cp24Time2a {
        milliseconds: 59999,
        minute: 59,
        invalid: true,
    };
    assert_eq!(
        decode_cp24time2a(&encode_cp24time2a(&cp24).expect("编码应成功")),
        cp24
    );
    assert_eq!(
        encode_cp24time2a(&Cp24Time2a {
            milliseconds: 60000,
            minute: 0,
            invalid: false
        }),
        Err(Asdu102Error::ValueOutOfRange)
    );

    let cp56 = Cp56Time2a {
        milliseconds: 59999,
        minute: 59,
        hour: 23,
        day: 31,
        month: 12,
        year: 2127,
        invalid: true,
        summer_time: true,
    };
    assert_eq!(
        decode_cp56time2a(&encode_cp56time2a(&cp56).expect("编码应成功")),
        cp56
    );
    assert_eq!(
        encode_cp56time2a(&Cp56Time2a {
            milliseconds: 0,
            minute: 0,
            hour: 24,
            day: 1,
            month: 1,
            year: 2026,
            invalid: false,
            summer_time: false
        }),
        Err(Asdu102Error::ValueOutOfRange)
    );
}
