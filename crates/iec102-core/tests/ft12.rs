//! FT1.2 帧编解码集成测试。

use iec102_core::ft12::{
    checksum, encode_frame, parse_frame, ControlField, EncodeError, Frame, LinkConfig, ParseError,
};

/// 解析恰好一帧，返回帧和消耗的字节数。
fn parse_exact(buf: &[u8], config: &LinkConfig) -> (Frame, usize) {
    let (frame, used) = parse_frame(buf, config).expect("解析失败");
    assert_eq!(used, buf.len(), "应恰好消耗整个缓冲区");
    (frame, used)
}

/// 编码一帧，断言成功。
fn encode_exact(frame: &Frame, config: &LinkConfig) -> Vec<u8> {
    encode_frame(frame, config).expect("编码失败")
}

#[test]
fn ack_encode_and_parse() {
    let cfg = LinkConfig::new(1);
    let encoded = encode_exact(&Frame::Ack, &cfg);
    assert_eq!(encoded, vec![0xE5]);
    assert_eq!(parse_exact(&encoded, &cfg), (Frame::Ack, 1));
}

#[test]
fn ack_consumes_one_byte_when_followed_by_frame() {
    let cfg = LinkConfig::new(1);
    let fixed = encode_exact(&Frame::Fixed { control: 0x40, address: 0x01 }, &cfg);
    let mut buf = vec![0xE5];
    buf.extend_from_slice(&fixed);

    let (frame, used) = parse_frame(&buf, &cfg).unwrap();
    assert_eq!(frame, Frame::Ack);
    assert_eq!(used, 1);
    assert_eq!(parse_exact(&buf[used..], &cfg), (Frame::Fixed { control: 0x40, address: 0x01 }, fixed.len()));
}

#[test]
fn fixed_frame_roundtrip_one_byte_address() {
    let cfg = LinkConfig::new(1);
    let frame = Frame::Fixed {
        control: 0x40,
        address: 0x01,
    };
    let encoded = encode_exact(&frame, &cfg);
    let expected_cs = checksum(&[0x40, 0x01]);
    assert_eq!(encoded, vec![0x10, 0x40, 0x01, expected_cs, 0x16]);
    assert_eq!(parse_exact(&encoded, &cfg), (frame, 5));
}

#[test]
fn fixed_frame_roundtrip_two_byte_address() {
    let cfg = LinkConfig::new(2);
    let frame = Frame::Fixed {
        control: 0x6B,
        address: 0x1234,
    };
    let encoded = encode_exact(&frame, &cfg);
    // 地址低字节在前。
    let expected_cs = checksum(&[0x6B, 0x34, 0x12]);
    assert_eq!(encoded, vec![0x10, 0x6B, 0x34, 0x12, expected_cs, 0x16]);
    assert_eq!(parse_exact(&encoded, &cfg), (frame, 6));
}

#[test]
fn variable_frame_roundtrip_one_byte_asdu() {
    let cfg = LinkConfig::new(1);
    let frame = Frame::Variable {
        control: 0x6B,
        address: 0x01,
        asdu: vec![0x64],
    };
    let encoded = encode_exact(&frame, &cfg);
    let length = 1 + 1 + 1; // 控制域 + 地址域 + ASDU
    assert_eq!(encoded.len(), 7 + length);
    assert_eq!(encoded[0], 0x68);
    // 第 2 到第 5 字节冗余发送同一个 L。
    assert_eq!(encoded[1], length as u8);
    assert_eq!(encoded[2], length as u8);
    assert_eq!(encoded[3], length as u8);
    assert_eq!(encoded[4], length as u8);
    assert_eq!(encoded[5], 0x6B);
    assert_eq!(encoded[6], 0x01);
    assert_eq!(encoded[7], 0x64);
    let expected_cs = checksum(&[0x6B, 0x01, 0x64]);
    assert_eq!(encoded[8], expected_cs);
    assert_eq!(encoded[9], 0x16);
    assert_eq!(parse_exact(&encoded, &cfg), (frame, encoded.len()));
}

#[test]
fn variable_frame_roundtrip_100_byte_asdu() {
    let cfg = LinkConfig::new(2);
    let asdu: Vec<u8> = (0..100u16).map(|i| (i % 251) as u8).collect();
    let frame = Frame::Variable {
        control: 0x00,
        address: 0xBEEF,
        asdu,
    };
    let encoded = encode_exact(&frame, &cfg);
    let length = 1 + 2 + 100;
    assert_eq!(encoded.len(), 7 + length);
    // 四个长度字节都等于 L。
    assert_eq!(encoded[1], length as u8);
    assert_eq!(encoded[2], length as u8);
    assert_eq!(encoded[3], length as u8);
    assert_eq!(encoded[4], length as u8);
    // 地址低字节在前：0xEF, 0xBE。
    assert_eq!(encoded[6], 0xEF);
    assert_eq!(encoded[7], 0xBE);
    let expected_cs = checksum(&encoded[5..5 + length]);
    assert_eq!(encoded[5 + length], expected_cs);
    assert_eq!(encoded[6 + length], 0x16);
    assert_eq!(parse_exact(&encoded, &cfg), (frame, encoded.len()));
}

#[test]
fn hand_crafted_variable_frame_hex() {
    let cfg = LinkConfig::new(1);
    let frame = Frame::Variable {
        control: 0x73,
        address: 0x01,
        asdu: vec![0x64, 0x01],
    };
    let encoded = encode_exact(&frame, &cfg);
    // L = 1（控制域）+ 1（地址域）+ 2（ASDU）= 4。
    let expected_cs = checksum(&[0x73, 0x01, 0x64, 0x01]);
    assert_eq!(
        encoded,
        vec![0x68, 0x04, 0x04, 0x04, 0x04, 0x73, 0x01, 0x64, 0x01, expected_cs, 0x16]
    );
    assert_eq!(parse_exact(&encoded, &cfg), (frame, encoded.len()));
}

#[test]
fn inconsistent_length_bytes_are_bad_length() {
    let cfg = LinkConfig::new(1);
    // 基础帧的四个长度字节都是 0x03，逐个改成 0x04 验证严格校验。
    for bad_index in 2..=4 {
        let mut buf = vec![0x68, 0x03, 0x03, 0x03, 0x03, 0x73, 0x01, 0x64, 0x01, 0x05, 0x16];
        buf[bad_index] = 0x04;
        assert_eq!(
            parse_frame(&buf, &cfg),
            Err(ParseError::BadLength),
            "第 {} 字节与第 2 字节不一致应返回 BadLength",
            bad_index
        );
    }
}

#[test]
fn variable_frame_too_long_returns_error() {
    let cfg = LinkConfig::new(1);
    // 1（控制域）+ 1（地址域）+ 254 = 256，超过 255。
    let frame = Frame::Variable {
        control: 0x6B,
        address: 0x01,
        asdu: vec![0x00; 254],
    };
    assert_eq!(encode_frame(&frame, &cfg), Err(EncodeError::FrameTooLong));

    // 边界：1 + 1 + 253 = 255，刚好可以编码。
    let frame = Frame::Variable {
        control: 0x6B,
        address: 0x01,
        asdu: vec![0x00; 253],
    };
    let encoded = encode_exact(&frame, &cfg);
    assert_eq!(encoded[1], 255);
    assert_eq!(encoded[2], 255);
    assert_eq!(encoded[3], 255);
    assert_eq!(encoded[4], 255);
    assert_eq!(encoded.len(), 7 + 255);
}

#[test]
fn encode_with_invalid_address_length_returns_error() {
    for len in [0u8, 3, 255] {
        let cfg = LinkConfig::new(len);
        assert_eq!(
            encode_frame(&Frame::Fixed { control: 0x40, address: 0x01 }, &cfg),
            Err(EncodeError::InvalidAddressLength)
        );
        assert_eq!(
            encode_frame(
                &Frame::Variable {
                    control: 0x6B,
                    address: 0x01,
                    asdu: vec![0x01],
                },
                &cfg
            ),
            Err(EncodeError::InvalidAddressLength)
        );
        // 单字符帧不受地址域长度影响。
        assert_eq!(encode_frame(&Frame::Ack, &cfg), Ok(vec![0xE5]));
    }
}

#[test]
fn bad_checksum_fixed_frame_reports_expected_and_actual() {
    let cfg = LinkConfig::new(1);
    let frame = Frame::Fixed {
        control: 0x40,
        address: 0x01,
    };
    let mut encoded = encode_exact(&frame, &cfg);
    let expected = encoded[3];
    let actual = expected.wrapping_add(1);
    encoded[3] = actual;
    assert_eq!(
        parse_frame(&encoded, &cfg),
        Err(ParseError::BadChecksum { expected, actual })
    );
}

#[test]
fn bad_checksum_variable_frame_reports_expected_and_actual() {
    let cfg = LinkConfig::new(2);
    let frame = Frame::Variable {
        control: 0x6B,
        address: 0x0001,
        asdu: vec![0x01, 0x02],
    };
    let mut encoded = encode_exact(&frame, &cfg);
    let index = encoded.len() - 2;
    let expected = encoded[index];
    let actual = expected.wrapping_add(1);
    encoded[index] = actual;
    assert_eq!(
        parse_frame(&encoded, &cfg),
        Err(ParseError::BadChecksum { expected, actual })
    );
}

#[test]
fn bad_end_byte_fixed_frame() {
    let cfg = LinkConfig::new(1);
    let frame = Frame::Fixed {
        control: 0x40,
        address: 0x01,
    };
    let mut encoded = encode_exact(&frame, &cfg);
    let last = encoded.len() - 1;
    encoded[last] = 0x00;
    assert_eq!(parse_frame(&encoded, &cfg), Err(ParseError::BadEndByte));
}

#[test]
fn bad_end_byte_variable_frame() {
    let cfg = LinkConfig::new(1);
    let frame = Frame::Variable {
        control: 0x6B,
        address: 0x01,
        asdu: vec![0x01],
    };
    let mut encoded = encode_exact(&frame, &cfg);
    let last = encoded.len() - 1;
    encoded[last] = 0xE5;
    assert_eq!(parse_frame(&encoded, &cfg), Err(ParseError::BadEndByte));
}

#[test]
fn bad_length_smaller_than_content() {
    let cfg = LinkConfig::new(1);
    // L = 0，但控制域加地址域至少需要 2 字节。
    let buf = vec![0x68, 0x00, 0x00, 0x00, 0x00, 0x40, 0x01, 0x41, 0x16];
    assert_eq!(parse_frame(&buf, &cfg), Err(ParseError::BadLength));
}

#[test]
fn bad_length_smaller_than_content_two_byte_address() {
    let cfg = LinkConfig::new(2);
    // L = 2，但控制域加 2 字节地址至少需要 3 字节。
    let buf = vec![0x68, 0x02, 0x02, 0x02, 0x02, 0x40, 0x01, 0x02, 0x00, 0x16];
    assert_eq!(parse_frame(&buf, &cfg), Err(ParseError::BadLength));
}

#[test]
fn incomplete_frames() {
    let cfg = LinkConfig::new(1);
    assert_eq!(parse_frame(&[], &cfg), Err(ParseError::Incomplete));

    let fixed = encode_exact(&Frame::Fixed { control: 0x40, address: 0x01 }, &cfg);
    assert_eq!(parse_frame(&fixed[..2], &cfg), Err(ParseError::Incomplete));

    let variable = encode_exact(
        &Frame::Variable {
            control: 0x6B,
            address: 0x01,
            asdu: vec![0x01, 0x02, 0x03],
        },
        &cfg,
    );
    // 只给前 3 字节。
    assert_eq!(parse_frame(&variable[..3], &cfg), Err(ParseError::Incomplete));
    // 只给启动字符和长度域。
    assert_eq!(parse_frame(&variable[..5], &cfg), Err(ParseError::Incomplete));
}

#[test]
fn two_frames_back_to_back() {
    let cfg = LinkConfig::new(1);
    let first = Frame::Fixed {
        control: 0x40,
        address: 0x01,
    };
    let second = Frame::Variable {
        control: 0x6B,
        address: 0x02,
        asdu: vec![0x01],
    };
    let first_bytes = encode_exact(&first, &cfg);
    let second_bytes = encode_exact(&second, &cfg);
    let mut buf = first_bytes.clone();
    buf.extend_from_slice(&second_bytes);

    let (frame, used) = parse_frame(&buf, &cfg).unwrap();
    assert_eq!(frame, first);
    assert_eq!(used, first_bytes.len());

    let (frame, used) = parse_frame(&buf[first_bytes.len()..], &cfg).unwrap();
    assert_eq!(frame, second);
    assert_eq!(used, second_bytes.len());
}

#[test]
fn invalid_address_length() {
    for len in [0u8, 3, 255] {
        let cfg = LinkConfig::new(len);
        let fixed = [0x10, 0x40, 0x01, 0x41, 0x16];
        assert_eq!(
            parse_frame(&fixed, &cfg),
            Err(ParseError::InvalidAddressLength)
        );
        let variable = [0x68, 0x03, 0x03, 0x03, 0x03, 0x40, 0x01, 0x01, 0x41, 0x16];
        assert_eq!(
            parse_frame(&variable, &cfg),
            Err(ParseError::InvalidAddressLength)
        );
    }
}

#[test]
fn bad_start_byte() {
    let cfg = LinkConfig::new(1);
    let buf = [0x00, 0x40, 0x01, 0x41, 0x16];
    assert_eq!(parse_frame(&buf, &cfg), Err(ParseError::BadStartByte));
}

#[test]
fn checksum_sum_low_byte() {
    assert_eq!(checksum(&[]), 0);
    assert_eq!(checksum(&[0x40, 0x01]), 0x41);
    assert_eq!(checksum(&[0xFF, 0x01]), 0x00);
    assert_eq!(checksum(&[0xFF, 0xFF]), 0xFE);
}

#[test]
fn control_field_roundtrip_all_bytes() {
    for value in 0u16..=255 {
        let field = ControlField::parse(value as u8);
        assert_eq!(
            field.encode(),
            value as u8,
            "控制域 0x{:02X} 往返失败",
            value
        );
    }
}

#[test]
fn control_field_parse_primary() {
    // 0xEB = 1110 1011：dir=1, prm=1, fcb=1, fcv=0, function=0xB。
    let field = ControlField::parse(0xEB);
    assert!(field.dir);
    assert!(field.prm);
    assert!(field.fcb);
    assert!(!field.fcv);
    assert_eq!(field.function, 0x0B);
    assert_eq!(field.encode(), 0xEB);
}

#[test]
fn control_field_parse_secondary() {
    // 0x30 = 0011 0000：prm=0, dfc=1, acd=1, function=0。
    let field = ControlField::parse(0x30);
    assert!(!field.dir);
    assert!(!field.prm);
    assert!(field.dfc);
    assert!(field.acd);
    assert_eq!(field.function, 0x00);
    assert_eq!(field.encode(), 0x30);
}

#[test]
fn control_field_encode_primary() {
    let field = ControlField {
        dir: true,
        prm: true,
        fcb: true,
        fcv: false,
        acd: false,
        dfc: false,
        function: 0x03,
    };
    assert_eq!(field.encode(), 0x80 | 0x40 | 0x20 | 0x03);
}

#[test]
fn control_field_encode_secondary() {
    let field = ControlField {
        dir: false,
        prm: false,
        fcb: false,
        fcv: false,
        acd: true,
        dfc: true,
        function: 0x09,
    };
    assert_eq!(field.encode(), 0x10 | 0x20 | 0x09);
}
