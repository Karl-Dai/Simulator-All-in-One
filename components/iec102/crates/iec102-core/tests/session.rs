//! 102 主站与子站会话集成测试。
//!
//! 主站与子站在内存里互发帧，不经过网络或串口。

use iec102_core::asdu::{encode_asdu102, Asdu102, InformationObject102, Qcc, StationAddress};
use iec102_core::ft12::{ControlField, Frame};
use iec102_core::link::LinkParams;
use iec102_core::session::{
    CounterPoint, MasterEvent102, MasterStation102, SlaveStation102, StationConfig102,
    StationEvent102,
};

const LINK_ADDRESS: u16 = 1;

fn config() -> StationConfig102 {
    StationConfig102 {
        link: LinkParams {
            address: LINK_ADDRESS,
            address_len: 1,
            max_retries: 3,
        },
        address: StationAddress {
            common_address: 1,
            ca_len: 1,
            ioa_len: 2,
        },
        poll_interval_ms: 1000,
        response_timeout_ms: 3000,
    }
}

fn slave(points: Vec<CounterPoint>) -> SlaveStation102 {
    SlaveStation102::new(config(), points)
}

/// 构造一帧累计量召唤（功能码 2，FCB=1）。
fn counter_interrogation_frame(qcc: Qcc) -> Frame {
    let asdu = Asdu102 {
        type_id: 120,
        sq: false,
        cause: 6,
        negative: false,
        test: false,
        originator: 0,
        address: config().address,
        objects: vec![InformationObject102::CounterInterrogation { ioa: 0, qcc }],
    };
    Frame::Variable {
        control: ControlField {
            dir: false,
            prm: true,
            fcb: true,
            fcv: true,
            acd: false,
            dfc: false,
            function: 2,
        }
        .encode(),
        address: LINK_ADDRESS,
        asdu: encode_asdu102(&asdu).expect("编码应成功"),
    }
}

/// 主站发一帧、子站应答、主站处理，完成一轮收发。
fn step(m: &mut MasterStation102, s: &mut SlaveStation102) {
    if let Some(frame) = m.next_frame() {
        if let Some(reply) = s.handle_frame(&frame) {
            m.handle_frame(&reply);
        }
    }
}

#[test]
fn duplicate_frame_executes_business_once() {
    let mut s = slave(vec![CounterPoint {
        ioa: 1,
        reading: 100,
    }]);

    let frame = counter_interrogation_frame(Qcc {
        request: 0,
        freeze: 0,
    });

    // 第一次投递：新帧，执行累计量召唤并应答。
    let first = s.handle_frame(&frame).expect("子站应应答");
    assert_eq!(s.stats().interrogations, 1);

    // 同一帧再投一次：判为重复帧，重发同一应答，不重复执行业务。
    let second = s.handle_frame(&frame).expect("重复帧应重发应答");
    assert_eq!(first, second);
    assert_eq!(
        s.stats().interrogations, 1,
        "重复帧不应重复执行累计量召唤"
    );
}

#[test]
fn ack_timeout_retransmits_then_fails() {
    let mut m = MasterStation102::new(config());
    let mut s = slave(vec![]);

    step(&mut m, &mut s);
    assert!(m.interrogate(Qcc {
        request: 0,
        freeze: 0
    }));
    let frame = m.next_frame().expect("主站应发出召唤帧");

    let timeout = config().response_timeout_ms;
    // 未到超时时间不重发。
    assert!(m.tick(timeout - 1).is_none());

    // 超过超时时间重发同一帧。
    let resent = m.tick(1).expect("超时后应重发");
    assert_eq!(resent, frame);

    // 连续重发直到达到上限。
    for _ in 1..config().link.max_retries {
        let resent = m.tick(timeout).expect("超时应重发");
        assert_eq!(resent, frame);
    }

    // 达到上限后不再重发，链路复位并产生命令失败事件。
    assert!(m.tick(timeout).is_none());
    assert!(m
        .drain_events()
        .iter()
        .any(|e| matches!(e, MasterEvent102::CommandFailed { .. })));

    // 链路回到 Reset，下一帧是复位链路帧。
    match m.next_frame() {
        Some(Frame::Fixed { control, .. }) => {
            assert_eq!(ControlField::parse(control).function, 0);
        }
        other => panic!("应为复位链路帧，实际 {other:?}"),
    }
}

#[test]
fn common_address_mismatch_is_rejected() {
    let mut s = slave(vec![CounterPoint {
        ioa: 1,
        reading: 100,
    }]);

    // 构造公共地址不匹配的累计量召唤 ASDU。
    let wrong = Asdu102 {
        type_id: 120,
        sq: false,
        cause: 6,
        negative: false,
        test: false,
        originator: 0,
        address: StationAddress {
            common_address: 99,
            ca_len: 1,
            ioa_len: 2,
        },
        objects: vec![InformationObject102::CounterInterrogation {
            ioa: 0,
            qcc: Qcc {
                request: 0,
                freeze: 0,
            },
        }],
    };
    let frame = Frame::Variable {
        control: ControlField {
            dir: false,
            prm: true,
            fcb: true,
            fcv: true,
            acd: false,
            dfc: false,
            function: 2,
        }
        .encode(),
        address: LINK_ADDRESS,
        asdu: encode_asdu102(&wrong).expect("编码应成功"),
    };
    let _ = s.handle_frame(&frame);
    assert_eq!(
        s.stats().interrogations, 0,
        "公共地址不匹配时不应执行累计量召唤"
    );
    assert_eq!(s.stats().errors, 1, "应记录一次错误");
    assert!(s
        .drain_events()
        .contains(&StationEvent102::CommonAddressMismatch {
            expected: 1,
            actual: 99,
        }));
}
