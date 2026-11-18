//! ASDU 编解码测试。

use iec101_core::asdu::{
    decode_asdu, decode_cp56time2a, encode_asdu, encode_cp56time2a, Asdu, AsduAddress, AsduError,
    Cp56Time2a, InformationObject, Quality,
};

fn quality(blocked: bool, substituted: bool, not_topical: bool, invalid: bool) -> Quality {
    Quality {
        blocked,
        substituted,
        not_topical,
        invalid,
    }
}

fn make_asdu(type_id: u8, objects: Vec<InformationObject>) -> Asdu {
    Asdu {
        type_id,
        sq: false,
        cause: 3,
        negative: false,
        test: false,
        originator: 0,
        address: AsduAddress {
            common_address: 1,
            ca_len: 1,
            ioa_len: 2,
        },
        objects,
    }
}

fn test_time() -> Cp56Time2a {
    Cp56Time2a {
        milliseconds: 12345,
        minute: 4,
        hour: 3,
        day: 2,
        month: 1,
        year: 2026,
        invalid: false,
        summer_time: false,
    }
}

/// 类型 1、SQ=0、两个信息对象、CA=1、IOA=2，逐字节核对。
#[test]
fn encode_type1_sq0_fixed_bytes() {
    let asdu = Asdu {
        type_id: 1,
        sq: false,
        cause: 3,
        negative: false,
        test: false,
        originator: 0,
        address: AsduAddress {
            common_address: 1,
            ca_len: 1,
            ioa_len: 2,
        },
        objects: vec![
            InformationObject::SinglePoint {
                ioa: 0x0102,
                value: true,
                quality: quality(true, false, false, false),
            },
            InformationObject::SinglePoint {
                ioa: 0x0304,
                value: false,
                quality: quality(false, false, false, true),
            },
        ],
    };
    let bytes = encode_asdu(&asdu).expect("编码应成功");
    assert_eq!(
        bytes,
        vec![
            0x01, // 类型标识 M_SP_NA_1
            0x02, // VSQ：SQ=0，数目=2
            0x03, // 传送原因：T=0，P/N=0，原因=3
            0x00, // OA
            0x01, // CA=1（1 字节）
            0x02, 0x01, // IOA=0x0102（2 字节，低字节在前）
            0x11, // SIQ：SPI=1，BL=1
            0x04, 0x03, // IOA=0x0304
            0x80, // SIQ：SPI=0，IV=1
        ]
    );
}

/// 固定字节解码后信息对象与品质位正确。
#[test]
fn decode_type1_fixed_bytes() {
    let bytes = [
        0x01, 0x02, 0x03, 0x00, 0x01, 0x02, 0x01, 0x11, 0x04, 0x03, 0x80,
    ];
    let address = AsduAddress {
        common_address: 1,
        ca_len: 1,
        ioa_len: 2,
    };
    let (asdu, consumed) = decode_asdu(&bytes, address).expect("解码应成功");
    assert_eq!(consumed, bytes.len());
    assert_eq!(asdu.type_id, 1);
    assert_eq!(asdu.cause, 3);
    assert_eq!(asdu.objects.len(), 2);
    assert_eq!(
        asdu.objects[0],
        InformationObject::SinglePoint {
            ioa: 0x0102,
            value: true,
            quality: quality(true, false, false, false),
        }
    );
    assert_eq!(
        asdu.objects[1],
        InformationObject::SinglePoint {
            ioa: 0x0304,
            value: false,
            quality: quality(false, false, false, true),
        }
    );
}

/// 类型 3、9、11、13 编码解码往返，含负数与浮点。
#[test]
fn roundtrip_measurement_types() {
    let address = AsduAddress {
        common_address: 7,
        ca_len: 1,
        ioa_len: 3,
    };
    let cases = vec![
        (
            3,
            InformationObject::DoublePoint {
                ioa: 1,
                value: 2,
                quality: quality(false, true, false, false),
            },
        ),
        (
            9,
            InformationObject::Normalized {
                ioa: 2,
                value: -1234,
                quality: quality(false, false, true, false),
            },
        ),
        (
            11,
            InformationObject::Scaled {
                ioa: 3,
                value: -32768,
                quality: quality(false, false, false, true),
            },
        ),
        (
            13,
            InformationObject::ShortFloat {
                ioa: 4,
                value: -1.5,
                quality: quality(true, false, false, false),
            },
        ),
    ];
    for (type_id, object) in cases {
        let asdu = Asdu {
            type_id,
            objects: vec![object.clone()],
            address,
            ..make_asdu(type_id, vec![])
        };
        let bytes = encode_asdu(&asdu).expect("编码应成功");
        let (decoded, consumed) = decode_asdu(&bytes, address).expect("解码应成功");
        assert_eq!(consumed, bytes.len());
        assert_eq!(decoded.type_id, type_id);
        assert_eq!(decoded.objects, vec![object]);
    }
}

/// 类型 30、31、34、35、36 往返，核对 CP56Time2a 的 7 个字节。
#[test]
fn roundtrip_time_tagged_types() {
    let time = test_time();
    let time_bytes = encode_cp56time2a(&time);
    // 12345 = 0x3039，低字节在前；2026 - 2000 = 26。
    assert_eq!(time_bytes, [0x39, 0x30, 0x04, 0x03, 0x02, 0x01, 0x1A]);

    let address = AsduAddress {
        common_address: 9,
        ca_len: 1,
        ioa_len: 2,
    };
    let cases = vec![
        (
            30,
            InformationObject::SinglePointTime {
                ioa: 5,
                value: true,
                quality: quality(false, false, false, false),
                time,
            },
        ),
        (
            31,
            InformationObject::DoublePointTime {
                ioa: 6,
                value: 3,
                quality: quality(false, false, false, false),
                time,
            },
        ),
        (
            34,
            InformationObject::NormalizedTime {
                ioa: 7,
                value: -1,
                quality: quality(false, false, false, false),
                time,
            },
        ),
        (
            35,
            InformationObject::ScaledTime {
                ioa: 8,
                value: -2,
                quality: quality(false, false, false, false),
                time,
            },
        ),
        (
            36,
            InformationObject::ShortFloatTime {
                ioa: 9,
                value: 3.5,
                quality: quality(false, false, false, false),
                time,
            },
        ),
    ];
    for (type_id, object) in cases {
        let asdu = Asdu {
            type_id,
            objects: vec![object.clone()],
            address,
            ..make_asdu(type_id, vec![])
        };
        let bytes = encode_asdu(&asdu).expect("编码应成功");
        // 头部 4 + CA 1 + IOA 2 = 7，时标应在倒数 7 字节。
        assert_eq!(&bytes[bytes.len() - 7..], &time_bytes);
        let (decoded, consumed) = decode_asdu(&bytes, address).expect("解码应成功");
        assert_eq!(consumed, bytes.len());
        assert_eq!(decoded.objects, vec![object]);
    }
}

/// SQ=1 连续地址：3 个信息对象只写第一个地址，解码后依次加 1。
#[test]
fn sq1_consecutive_addresses() {
    let address = AsduAddress {
        common_address: 1,
        ca_len: 1,
        ioa_len: 2,
    };
    let asdu = Asdu {
        type_id: 1,
        sq: true,
        cause: 3,
        negative: false,
        test: false,
        originator: 0,
        address,
        objects: vec![
            InformationObject::SinglePoint {
                ioa: 10,
                value: true,
                quality: quality(false, false, false, false),
            },
            InformationObject::SinglePoint {
                ioa: 11,
                value: false,
                quality: quality(false, false, false, false),
            },
            InformationObject::SinglePoint {
                ioa: 12,
                value: true,
                quality: quality(false, false, false, false),
            },
        ],
    };
    let bytes = encode_asdu(&asdu).expect("编码应成功");
    // 头部 4 + CA 1 + IOA 2 + 3 个元素 = 10。
    assert_eq!(bytes.len(), 10);
    assert_eq!(&bytes[5..7], &[10, 0]);
    assert_eq!(bytes[1] & 0x80, 0x80, "SQ 位应为 1");
    assert_eq!(bytes[1] & 0x7F, 3, "信息对象数目应为 3");

    let (decoded, consumed) = decode_asdu(&bytes, address).expect("解码应成功");
    assert_eq!(consumed, bytes.len());
    assert!(decoded.sq);
    assert_eq!(decoded.objects.len(), 3);
    assert_eq!(decoded.objects[0].ioa(), 10);
    assert_eq!(decoded.objects[1].ioa(), 11);
    assert_eq!(decoded.objects[2].ioa(), 12);
}

/// 传送原因 T 位、P/N 位、OA 值往返。
#[test]
fn cause_bits_roundtrip() {
    let address = AsduAddress {
        common_address: 1,
        ca_len: 1,
        ioa_len: 2,
    };
    let asdu = Asdu {
        type_id: 1,
        sq: false,
        cause: 20,
        negative: true,
        test: true,
        originator: 5,
        address,
        objects: vec![InformationObject::SinglePoint {
            ioa: 1,
            value: true,
            quality: quality(false, false, false, false),
        }],
    };
    let bytes = encode_asdu(&asdu).expect("编码应成功");
    // T=1、P/N=1、原因=20 → 0x80 | 0x40 | 20 = 0xD4。
    assert_eq!(bytes[2], 0xD4);
    assert_eq!(bytes[3], 5);

    let (decoded, _) = decode_asdu(&bytes, address).expect("解码应成功");
    assert!(decoded.test);
    assert!(decoded.negative);
    assert_eq!(decoded.cause, 20);
    assert_eq!(decoded.originator, 5);
}

/// 公共地址 2 字节、信息对象地址 3 字节往返。
#[test]
fn two_byte_ca_three_byte_ioa() {
    let address = AsduAddress {
        common_address: 0x1234,
        ca_len: 2,
        ioa_len: 3,
    };
    let asdu = Asdu {
        type_id: 1,
        sq: false,
        cause: 3,
        negative: false,
        test: false,
        originator: 0,
        address,
        objects: vec![InformationObject::SinglePoint {
            ioa: 0x123456,
            value: true,
            quality: quality(false, false, false, false),
        }],
    };
    let bytes = encode_asdu(&asdu).expect("编码应成功");
    assert_eq!(&bytes[4..6], &[0x34, 0x12], "CA 低字节在前");
    assert_eq!(&bytes[6..9], &[0x56, 0x34, 0x12], "IOA 低字节在前");
    // 解码时传入的 common_address 不参与解析，结果应取自报文。
    let decode_config = AsduAddress {
        common_address: 0,
        ca_len: 2,
        ioa_len: 3,
    };
    let (decoded, consumed) = decode_asdu(&bytes, decode_config).expect("解码应成功");
    assert_eq!(consumed, bytes.len());
    assert_eq!(decoded.address.common_address, 0x1234);
    assert_eq!(decoded.objects[0].ioa(), 0x123456);
}

/// 截断的 ASDU 返回 TooShort 或 LengthMismatch。
#[test]
fn truncated_asdu_errors() {
    let address = AsduAddress {
        common_address: 1,
        ca_len: 1,
        ioa_len: 2,
    };
    assert!(matches!(
        decode_asdu(&[], address),
        Err(AsduError::TooShort)
    ));
    assert!(matches!(
        decode_asdu(&[0x01, 0x02, 0x03], address),
        Err(AsduError::TooShort)
    ));
    // 头部完整，声明 2 个对象但没有对象数据。
    let bytes = [0x01, 0x02, 0x03, 0x00, 0x01];
    assert!(matches!(
        decode_asdu(&bytes, address),
        Err(AsduError::LengthMismatch { .. })
    ));
    // 只有 1 个对象的数据。
    let bytes = [0x01, 0x02, 0x03, 0x00, 0x01, 0x02, 0x01, 0x11];
    assert!(matches!(
        decode_asdu(&bytes, address),
        Err(AsduError::LengthMismatch { .. })
    ));
}

/// 未知类型标识返回 UnknownType。
#[test]
fn unknown_type() {
    let address = AsduAddress {
        common_address: 1,
        ca_len: 1,
        ioa_len: 2,
    };
    let bytes = [0x63, 0x01, 0x03, 0x00, 0x01, 0x01, 0x00];
    assert!(matches!(
        decode_asdu(&bytes, address),
        Err(AsduError::UnknownType(0x63))
    ));
    let asdu = make_asdu(
        0x63,
        vec![InformationObject::SinglePoint {
            ioa: 1,
            value: true,
            quality: quality(false, false, false, false),
        }],
    );
    assert!(matches!(
        encode_asdu(&asdu),
        Err(AsduError::UnknownType(0x63))
    ));
}

/// decode_cp56time2a 解析 IV 位与 SU 位。
#[test]
fn cp56time2a_iv_su_bits() {
    let bytes = [0x00, 0x00, 0x80 | 4, 0x80 | 3, 0x02, 0x01, 0x1A];
    let time = decode_cp56time2a(&bytes);
    assert!(time.invalid, "bit7 of b2 是 IV");
    assert!(time.summer_time, "bit7 of b3 是 SU");
    assert_eq!(time.minute, 4);
    assert_eq!(time.hour, 3);
    assert_eq!(time.day, 2);
    assert_eq!(time.month, 1);
    assert_eq!(time.year, 2026);
    assert_eq!(time.milliseconds, 0);
}

/// 编码后再解码，对象数量、类型、值与品质位全部一致。
#[test]
fn roundtrip_preserves_all_fields() {
    let address = AsduAddress {
        common_address: 0x1234,
        ca_len: 2,
        ioa_len: 3,
    };
    let time = Cp56Time2a {
        milliseconds: 59999,
        minute: 59,
        hour: 23,
        day: 31,
        month: 12,
        year: 2127,
        invalid: true,
        summer_time: true,
    };
    let all = quality(true, true, true, true);
    let cases = vec![
        (
            1,
            InformationObject::SinglePoint {
                ioa: 0x010203,
                value: true,
                quality: all,
            },
        ),
        (
            3,
            InformationObject::DoublePoint {
                ioa: 0x010204,
                value: 3,
                quality: all,
            },
        ),
        (
            9,
            InformationObject::Normalized {
                ioa: 0x010205,
                value: -12345,
                quality: all,
            },
        ),
        (
            11,
            InformationObject::Scaled {
                ioa: 0x010206,
                value: -32768,
                quality: all,
            },
        ),
        (
            13,
            InformationObject::ShortFloat {
                ioa: 0x010207,
                value: -1.25,
                quality: all,
            },
        ),
        (
            30,
            InformationObject::SinglePointTime {
                ioa: 0x010208,
                value: false,
                quality: all,
                time,
            },
        ),
        (
            31,
            InformationObject::DoublePointTime {
                ioa: 0x010209,
                value: 1,
                quality: all,
                time,
            },
        ),
        (
            34,
            InformationObject::NormalizedTime {
                ioa: 0x01020A,
                value: 32767,
                quality: all,
                time,
            },
        ),
        (
            35,
            InformationObject::ScaledTime {
                ioa: 0x01020B,
                value: 0,
                quality: all,
                time,
            },
        ),
        (
            36,
            InformationObject::ShortFloatTime {
                ioa: 0x01020C,
                value: 0.5,
                quality: all,
                time,
            },
        ),
    ];
    for (type_id, object) in cases {
        let asdu = Asdu {
            type_id,
            sq: false,
            cause: 20,
            negative: true,
            test: false,
            originator: 9,
            address,
            objects: vec![object.clone()],
        };
        let bytes = encode_asdu(&asdu).expect("编码应成功");
        let (decoded, consumed) = decode_asdu(&bytes, address).expect("解码应成功");
        assert_eq!(consumed, bytes.len());
        assert_eq!(decoded.type_id, type_id);
        assert_eq!(decoded.cause, 20);
        assert!(decoded.negative);
        assert!(!decoded.test);
        assert_eq!(decoded.originator, 9);
        assert_eq!(decoded.objects.len(), 1);
        assert_eq!(decoded.objects[0], object);
    }
}

/// 编码时检查值范围。
#[test]
fn encode_value_range_checks() {
    // 双点值必须 0..=3。
    let asdu = make_asdu(
        3,
        vec![InformationObject::DoublePoint {
            ioa: 1,
            value: 4,
            quality: quality(false, false, false, false),
        }],
    );
    assert!(matches!(
        encode_asdu(&asdu),
        Err(AsduError::ValueOutOfRange)
    ));

    // 毫秒不能超过 59999。
    let asdu = make_asdu(
        30,
        vec![InformationObject::SinglePointTime {
            ioa: 1,
            value: true,
            quality: quality(false, false, false, false),
            time: Cp56Time2a {
                milliseconds: 60000,
                ..test_time()
            },
        }],
    );
    assert!(matches!(
        encode_asdu(&asdu),
        Err(AsduError::ValueOutOfRange)
    ));

    // IOA 超过 1 字节上限。
    let asdu = Asdu {
        address: AsduAddress {
            common_address: 1,
            ca_len: 1,
            ioa_len: 1,
        },
        ..make_asdu(
            1,
            vec![InformationObject::SinglePoint {
                ioa: 256,
                value: true,
                quality: quality(false, false, false, false),
            }],
        )
    };
    assert!(matches!(
        encode_asdu(&asdu),
        Err(AsduError::ValueOutOfRange)
    ));

    // CA 超过 1 字节上限。
    let asdu = Asdu {
        address: AsduAddress {
            common_address: 256,
            ca_len: 1,
            ioa_len: 2,
        },
        ..make_asdu(
            1,
            vec![InformationObject::SinglePoint {
                ioa: 1,
                value: true,
                quality: quality(false, false, false, false),
            }],
        )
    };
    assert!(matches!(
        encode_asdu(&asdu),
        Err(AsduError::ValueOutOfRange)
    ));
}

/// 信息对象数目为 0 时不合法。
#[test]
fn zero_object_count() {
    let asdu = make_asdu(1, vec![]);
    assert!(matches!(
        encode_asdu(&asdu),
        Err(AsduError::ObjectCountMismatch)
    ));
    let address = AsduAddress {
        common_address: 1,
        ca_len: 1,
        ioa_len: 2,
    };
    let bytes = [0x01, 0x00, 0x03, 0x00, 0x01];
    assert!(matches!(
        decode_asdu(&bytes, address),
        Err(AsduError::ObjectCountMismatch)
    ));
}

/// 非法地址长度返回 UnsupportedAddressLength。
#[test]
fn unsupported_address_length() {
    let asdu = Asdu {
        address: AsduAddress {
            common_address: 1,
            ca_len: 3,
            ioa_len: 1,
        },
        ..make_asdu(
            1,
            vec![InformationObject::SinglePoint {
                ioa: 1,
                value: true,
                quality: quality(false, false, false, false),
            }],
        )
    };
    assert!(matches!(
        encode_asdu(&asdu),
        Err(AsduError::UnsupportedAddressLength)
    ));
    assert!(matches!(
        decode_asdu(
            &[0x01, 0x01, 0x03, 0x00],
            AsduAddress {
                common_address: 0,
                ca_len: 0,
                ioa_len: 1,
            }
        ),
        Err(AsduError::UnsupportedAddressLength)
    ));
}

/// ASDU 之后的额外字节不影响解码，已消费字节数指向 ASDU 结束位置。
#[test]
fn extra_bytes_after_asdu() {
    let address = AsduAddress {
        common_address: 1,
        ca_len: 1,
        ioa_len: 2,
    };
    let asdu = make_asdu(
        1,
        vec![InformationObject::SinglePoint {
            ioa: 1,
            value: true,
            quality: quality(false, false, false, false),
        }],
    );
    let mut bytes = encode_asdu(&asdu).expect("编码应成功");
    let asdu_len = bytes.len();
    bytes.extend_from_slice(&[0xAA, 0xBB]);
    let (decoded, consumed) = decode_asdu(&bytes, address).expect("解码应成功");
    assert_eq!(consumed, asdu_len);
    assert_eq!(decoded.objects.len(), 1);
}

/// 类型 45 单命令：S/E 两种状态与 QU 值往返，逐字节核对。
#[test]
fn roundtrip_type45_single_command() {
    let address = AsduAddress {
        common_address: 1,
        ca_len: 1,
        ioa_len: 2,
    };
    let cases = vec![
        // (select, on, qualifier, 期望控制字节)
        (true, true, 0u8, 0x81u8),   // S/E=1, QU=0, SCS=1
        (false, false, 2u8, 0x40u8), // S/E=0, QU=2, SCS=0
        (true, false, 3u8, 0xE0u8),  // S/E=1, QU=3, SCS=0
        (false, true, 1u8, 0x21u8),  // S/E=0, QU=1, SCS=1
    ];
    for (select, on, qualifier, expected) in cases {
        let object = InformationObject::SingleCommand {
            ioa: 1,
            on,
            select,
            qualifier,
        };
        let asdu = Asdu {
            type_id: 45,
            objects: vec![object.clone()],
            address,
            ..make_asdu(45, vec![])
        };
        let bytes = encode_asdu(&asdu).expect("编码应成功");
        // 头部 4 + CA 1 + IOA 2 = 7，控制字节在索引 7。
        assert_eq!(bytes[7], expected);
        let (decoded, consumed) = decode_asdu(&bytes, address).expect("解码应成功");
        assert_eq!(consumed, bytes.len());
        assert_eq!(decoded.objects, vec![object]);
    }
}

/// 类型 46 双命令：value 0..=3 往返，越界返回 ValueOutOfRange。
#[test]
fn roundtrip_type46_double_command() {
    let address = AsduAddress {
        common_address: 1,
        ca_len: 1,
        ioa_len: 2,
    };
    for value in 0..=3u8 {
        let object = InformationObject::DoubleCommand {
            ioa: 2,
            value,
            select: true,
            qualifier: 1,
        };
        let asdu = Asdu {
            type_id: 46,
            objects: vec![object.clone()],
            address,
            ..make_asdu(46, vec![])
        };
        let bytes = encode_asdu(&asdu).expect("编码应成功");
        // S/E=1, QU=1, DCS=value → 0x80 | 0x20 | value。
        assert_eq!(bytes[7], 0x80 | 0x20 | value);
        let (decoded, consumed) = decode_asdu(&bytes, address).expect("解码应成功");
        assert_eq!(consumed, bytes.len());
        assert_eq!(decoded.objects, vec![object]);
    }
    let asdu = make_asdu(
        46,
        vec![InformationObject::DoubleCommand {
            ioa: 2,
            value: 4,
            select: false,
            qualifier: 0,
        }],
    );
    assert!(matches!(
        encode_asdu(&asdu),
        Err(AsduError::ValueOutOfRange)
    ));
}

/// 类型 47 步调节命令：value 0..=31 往返，越界返回 ValueOutOfRange。
#[test]
fn roundtrip_type47_step_command() {
    let address = AsduAddress {
        common_address: 1,
        ca_len: 1,
        ioa_len: 2,
    };
    for value in 0..=31u8 {
        let object = InformationObject::StepCommand {
            ioa: 3,
            value,
            select: false,
            qualifier: 2,
        };
        let asdu = Asdu {
            type_id: 47,
            objects: vec![object.clone()],
            address,
            ..make_asdu(47, vec![])
        };
        let bytes = encode_asdu(&asdu).expect("编码应成功");
        // S/E=0, QU=2, RCS=value → 0x40 | value。
        assert_eq!(bytes[7], 0x40 | value);
        let (decoded, consumed) = decode_asdu(&bytes, address).expect("解码应成功");
        assert_eq!(consumed, bytes.len());
        assert_eq!(decoded.objects, vec![object]);
    }
    let asdu = make_asdu(
        47,
        vec![InformationObject::StepCommand {
            ioa: 3,
            value: 32,
            select: false,
            qualifier: 0,
        }],
    );
    assert!(matches!(
        encode_asdu(&asdu),
        Err(AsduError::ValueOutOfRange)
    ));
}

/// 类型 48、49、50 设点命令往返，含负数与浮点，核对 QOS 字节。
#[test]
fn roundtrip_setpoint_types() {
    let address = AsduAddress {
        common_address: 1,
        ca_len: 1,
        ioa_len: 2,
    };
    let cases = vec![
        (
            48,
            InformationObject::SetpointNormalized {
                ioa: 10,
                value: -1234,
                select: true,
                qualifier: 5,
            },
            0x85u8, // S/E=1, QL=5
        ),
        (
            49,
            InformationObject::SetpointScaled {
                ioa: 11,
                value: -32768,
                select: false,
                qualifier: 127,
            },
            0x7Fu8, // S/E=0, QL=127
        ),
        (
            50,
            InformationObject::SetpointFloat {
                ioa: 12,
                value: -1.5,
                select: true,
                qualifier: 0,
            },
            0x80u8, // S/E=1, QL=0
        ),
    ];
    for (type_id, object, qos) in cases {
        let asdu = Asdu {
            type_id,
            objects: vec![object.clone()],
            address,
            ..make_asdu(type_id, vec![])
        };
        let bytes = encode_asdu(&asdu).expect("编码应成功");
        assert_eq!(bytes[bytes.len() - 1], qos, "QOS 字节不匹配");
        let (decoded, consumed) = decode_asdu(&bytes, address).expect("解码应成功");
        assert_eq!(consumed, bytes.len());
        assert_eq!(decoded.objects, vec![object]);
    }
}

/// 类型 100 总召唤：QOI=20 与 QOI=1 往返。
#[test]
fn roundtrip_type100_interrogation() {
    let address = AsduAddress {
        common_address: 1,
        ca_len: 1,
        ioa_len: 2,
    };
    for qoi in [1u8, 20u8] {
        let object = InformationObject::InterrogationCommand { ioa: 0, qoi };
        let asdu = Asdu {
            type_id: 100,
            objects: vec![object.clone()],
            address,
            ..make_asdu(100, vec![])
        };
        let bytes = encode_asdu(&asdu).expect("编码应成功");
        assert_eq!(bytes[7], qoi);
        let (decoded, consumed) = decode_asdu(&bytes, address).expect("解码应成功");
        assert_eq!(consumed, bytes.len());
        assert_eq!(decoded.objects, vec![object]);
    }
}

/// 类型 101 计数量召唤：四种 QCC 值往返。
#[test]
fn roundtrip_type101_counter_interrogation() {
    let address = AsduAddress {
        common_address: 1,
        ca_len: 1,
        ioa_len: 2,
    };
    // FRZ=0 读、1 冻结并读、2 冻结并复位读、3 复位读。
    for qcc in [0x00u8, 0x40u8, 0x80u8, 0xC0u8] {
        let object = InformationObject::CounterInterrogation { ioa: 0, qcc };
        let asdu = Asdu {
            type_id: 101,
            objects: vec![object.clone()],
            address,
            ..make_asdu(101, vec![])
        };
        let bytes = encode_asdu(&asdu).expect("编码应成功");
        assert_eq!(bytes[7], qcc);
        let (decoded, consumed) = decode_asdu(&bytes, address).expect("解码应成功");
        assert_eq!(consumed, bytes.len());
        assert_eq!(decoded.objects, vec![object]);
    }
}

/// 类型 103 时钟同步：CP56Time2a 往返。
#[test]
fn roundtrip_type103_clock_sync() {
    let address = AsduAddress {
        common_address: 1,
        ca_len: 1,
        ioa_len: 2,
    };
    let time = test_time();
    let object = InformationObject::ClockSync { ioa: 0, time };
    let asdu = Asdu {
        type_id: 103,
        objects: vec![object.clone()],
        address,
        ..make_asdu(103, vec![])
    };
    let bytes = encode_asdu(&asdu).expect("编码应成功");
    // 头部 4 + CA 1 + IOA 2 = 7，时标 7 字节在末尾。
    assert_eq!(&bytes[7..14], &encode_cp56time2a(&time));
    let (decoded, consumed) = decode_asdu(&bytes, address).expect("解码应成功");
    assert_eq!(consumed, bytes.len());
    assert_eq!(decoded.objects, vec![object]);
}

/// 类型 105 复位进程：QRP=0 与 QRP=2 往返。
#[test]
fn roundtrip_type105_reset_process() {
    let address = AsduAddress {
        common_address: 1,
        ca_len: 1,
        ioa_len: 2,
    };
    for qrp in [0u8, 2u8] {
        let object = InformationObject::ResetProcess { ioa: 0, qrp };
        let asdu = Asdu {
            type_id: 105,
            objects: vec![object.clone()],
            address,
            ..make_asdu(105, vec![])
        };
        let bytes = encode_asdu(&asdu).expect("编码应成功");
        assert_eq!(bytes[7], qrp);
        let (decoded, consumed) = decode_asdu(&bytes, address).expect("解码应成功");
        assert_eq!(consumed, bytes.len());
        assert_eq!(decoded.objects, vec![object]);
    }
}

/// select=true 时控制字节 bit7 为 1，解码后 select 一致。
#[test]
fn select_bit_set_in_control_byte() {
    let address = AsduAddress {
        common_address: 1,
        ca_len: 1,
        ioa_len: 2,
    };
    let cases = vec![
        (
            45,
            InformationObject::SingleCommand {
                ioa: 1,
                on: true,
                select: true,
                qualifier: 0,
            },
        ),
        (
            46,
            InformationObject::DoubleCommand {
                ioa: 2,
                value: 1,
                select: true,
                qualifier: 0,
            },
        ),
        (
            47,
            InformationObject::StepCommand {
                ioa: 3,
                value: 1,
                select: true,
                qualifier: 0,
            },
        ),
        (
            48,
            InformationObject::SetpointNormalized {
                ioa: 4,
                value: 1,
                select: true,
                qualifier: 0,
            },
        ),
        (
            49,
            InformationObject::SetpointScaled {
                ioa: 5,
                value: 1,
                select: true,
                qualifier: 0,
            },
        ),
        (
            50,
            InformationObject::SetpointFloat {
                ioa: 6,
                value: 1.0,
                select: true,
                qualifier: 0,
            },
        ),
    ];
    for (type_id, object) in cases {
        let asdu = Asdu {
            type_id,
            objects: vec![object.clone()],
            address,
            ..make_asdu(type_id, vec![])
        };
        let bytes = encode_asdu(&asdu).expect("编码应成功");
        let control = if type_id <= 47 {
            bytes[7]
        } else {
            bytes[bytes.len() - 1]
        };
        assert_eq!(control & 0x80, 0x80, "类型 {} 的 S/E 位应为 1", type_id);
        let (decoded, _) = decode_asdu(&bytes, address).expect("解码应成功");
        let select = match &decoded.objects[0] {
            InformationObject::SingleCommand { select, .. }
            | InformationObject::DoubleCommand { select, .. }
            | InformationObject::StepCommand { select, .. }
            | InformationObject::SetpointNormalized { select, .. }
            | InformationObject::SetpointScaled { select, .. }
            | InformationObject::SetpointFloat { select, .. } => *select,
            _ => false,
        };
        assert!(select, "类型 {} 解码后 select 应为 true", type_id);
    }
}

/// 未知类型标识仍返回 UnknownType。
#[test]
fn unknown_type_still_errors() {
    let address = AsduAddress {
        common_address: 1,
        ca_len: 1,
        ioa_len: 2,
    };
    // 102 不在支持范围内。
    let bytes = [0x66, 0x01, 0x03, 0x00, 0x01, 0x01, 0x00];
    assert!(matches!(
        decode_asdu(&bytes, address),
        Err(AsduError::UnknownType(102))
    ));
    let asdu = make_asdu(
        102,
        vec![InformationObject::SinglePoint {
            ioa: 1,
            value: true,
            quality: quality(false, false, false, false),
        }],
    );
    assert!(matches!(
        encode_asdu(&asdu),
        Err(AsduError::UnknownType(102))
    ));
}
