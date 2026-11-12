//! 主站与子站会话集成测试。
//!
//! 主站与子站在内存里互发帧，不经过网络或串口。

use iec101_core::asdu::{encode_asdu, Asdu, AsduAddress, Cp56Time2a, InformationObject};
use iec101_core::ft12::{ControlField, Frame};
use iec101_core::link::LinkParams;
use iec101_core::session::{
    MasterEvent101, MasterStation, PointKind, PointValue, SlaveStation, StationConfig, StationEvent,
};

const LINK_ADDRESS: u16 = 1;

fn config() -> StationConfig {
    StationConfig {
        link: LinkParams {
            address: LINK_ADDRESS,
            address_len: 1,
            max_retries: 3,
        },
        address: AsduAddress {
            common_address: 1,
            ca_len: 1,
            ioa_len: 2,
        },
        poll_interval_ms: 1000,
        response_timeout_ms: 3000,
    }
}

fn master() -> MasterStation {
    MasterStation::new(config())
}

fn slave(points: Vec<(u32, PointKind)>) -> SlaveStation {
    SlaveStation::new(config(), points)
}

/// 主站发一帧、子站应答、主站处理，完成一轮收发。
fn step(m: &mut MasterStation, s: &mut SlaveStation) {
    if let Some(frame) = m.next_frame() {
        if let Some(reply) = s.handle_frame(&frame) {
            m.handle_frame(&reply);
        }
    }
}

/// 返回 received 中某个 IOA 的最新值。
fn latest(received: &[(u32, PointValue)], ioa: u32) -> Option<PointValue> {
    received
        .iter()
        .rev()
        .find(|(i, _)| *i == ioa)
        .map(|(_, v)| *v)
}

#[test]
fn reset_link_then_master_gets_link_ok() {
    let mut m = master();
    let mut s = slave(vec![]);

    step(&mut m, &mut s);

    assert!(m.drain_events().contains(&MasterEvent101::LinkOk));
    assert!(s.drain_events().contains(&StationEvent::LinkReset));
}

#[test]
fn interrogation_flow_with_three_points() {
    let mut m = master();
    let mut s = slave(vec![
        (1, PointKind::SinglePoint),
        (2, PointKind::DoublePoint),
        (3, PointKind::ShortFloat),
    ]);

    step(&mut m, &mut s);
    assert!(m.interrogate(20));
    for _ in 0..20 {
        step(&mut m, &mut s);
    }

    let master_events = m.drain_events();
    let updates = master_events
        .iter()
        .filter(|e| matches!(e, MasterEvent101::PointUpdate { .. }))
        .count();
    assert_eq!(updates, 3, "主站应收到 3 个点位更新");
    assert!(master_events.contains(&MasterEvent101::InterrogationStarted));
    assert!(master_events.contains(&MasterEvent101::InterrogationFinished));

    let slave_events = s.drain_events();
    assert!(slave_events.contains(&StationEvent::InterrogationStarted { qoi: 20 }));
    assert!(slave_events.contains(&StationEvent::InterrogationFinished));
}

#[test]
fn interrogation_values_match_set_values() {
    let mut m = master();
    let mut s = slave(vec![
        (1, PointKind::SinglePoint),
        (2, PointKind::DoublePoint),
        (3, PointKind::ShortFloat),
    ]);

    step(&mut m, &mut s);
    assert!(s.set_value(1, PointValue::Bool(true)));
    assert!(s.set_value(2, PointValue::Double(1)));
    assert!(s.set_value(3, PointValue::Float(12.5)));

    assert!(m.interrogate(20));
    for _ in 0..20 {
        step(&mut m, &mut s);
    }

    let received = m.received();
    assert_eq!(latest(&received, 1), Some(PointValue::Bool(true)));
    assert_eq!(latest(&received, 2), Some(PointValue::Double(1)));
    assert_eq!(latest(&received, 3), Some(PointValue::Float(12.5)));
}

#[test]
fn forty_five_points_are_split_into_frames() {
    let points: Vec<(u32, PointKind)> = (1..=45)
        .map(|ioa| (ioa, PointKind::SinglePoint))
        .collect();
    let mut m = master();
    let mut s = slave(points);

    step(&mut m, &mut s);
    assert!(m.interrogate(20));
    for _ in 0..30 {
        step(&mut m, &mut s);
    }

    let updates = m
        .drain_events()
        .into_iter()
        .filter_map(|e| match e {
            MasterEvent101::PointUpdate { ioa, .. } => Some(ioa),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(updates.len(), 45, "主站应收到全部 45 个点位");
    let mut sorted = updates.clone();
    sorted.sort_unstable();
    assert_eq!(sorted, (1..=45).collect::<Vec<_>>());
}

#[test]
fn set_value_type_mismatch_then_spontaneous_update() {
    let mut m = master();
    let mut s = slave(vec![(1, PointKind::SinglePoint)]);

    step(&mut m, &mut s);

    // 类型不匹配返回 false。
    assert!(!s.set_value(1, PointValue::Double(1)));
    // 类型匹配返回 true。
    assert!(s.set_value(1, PointValue::Bool(true)));
    assert!(s.drain_events().contains(&StationEvent::PointChanged { ioa: 1 }));

    // 下一次轮询主站收到突发数据。
    step(&mut m, &mut s);
    assert!(m.drain_events().contains(&MasterEvent101::PointUpdate {
        ioa: 1,
        value: PointValue::Bool(true),
    }));
}

#[test]
fn repeated_interrogate_returns_false() {
    let mut m = master();
    let mut s = slave(vec![]);

    step(&mut m, &mut s);

    assert!(m.interrogate(20));
    assert!(!m.interrogate(20));
}

#[test]
fn unsupported_type_increments_errors_and_master_command_failed() {
    let mut m = master();
    let mut s = slave(vec![]);

    step(&mut m, &mut s);

    // 构造子站不支持的命令类型（单命令 C_SC_NA_1，类型 45）。
    let command = Asdu {
        type_id: 45,
        sq: false,
        cause: 6,
        negative: false,
        test: false,
        originator: 0,
        address: config().address,
        objects: vec![InformationObject::SingleCommand {
            ioa: 1,
            on: true,
            select: false,
            qualifier: 0,
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
        asdu: encode_asdu(&command).expect("编码应成功"),
    };
    let reply = s.handle_frame(&frame).expect("子站应应答");
    assert!(matches!(reply, Frame::Fixed { .. }));
    assert_eq!(s.stats().errors, 1, "子站错误计数应加 1");

    // 主站轮询拿到否定确认。
    step(&mut m, &mut s);
    assert!(m
        .drain_events()
        .iter()
        .any(|e| matches!(e, MasterEvent101::CommandFailed { .. })));
}

#[test]
fn control_executes_and_confirms() {
    let mut m = master();
    let mut s = slave(vec![(1, PointKind::SinglePoint)]);

    step(&mut m, &mut s);
    assert!(s.set_value(1, PointValue::Bool(true)));
    assert!(m.control(1, false));
    assert!(!m.control(1, true), "已有命令未完成时应返回 false");

    for _ in 0..20 {
        step(&mut m, &mut s);
    }

    assert_eq!(s.value(1), Some(PointValue::Bool(false)));
    assert!(m
        .drain_events()
        .contains(&MasterEvent101::ControlConfirmed { ioa: 1, on: false }));
    assert!(s
        .drain_events()
        .contains(&StationEvent::ControlConfirmed { ioa: 1, on: false }));
}

#[test]
fn select_command_does_not_change_value() {
    let mut m = master();
    let mut s = slave(vec![(1, PointKind::SinglePoint)]);

    step(&mut m, &mut s);
    assert!(s.set_value(1, PointValue::Bool(true)));

    // 直接构造 S/E=1 的选择命令帧发给子站。
    let select = Asdu {
        type_id: 45,
        sq: false,
        cause: 6,
        negative: false,
        test: false,
        originator: 0,
        address: config().address,
        objects: vec![InformationObject::SingleCommand {
            ioa: 1,
            on: false,
            select: true,
            qualifier: 0,
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
        asdu: encode_asdu(&select).expect("编码应成功"),
    };
    let reply = s.handle_frame(&frame).expect("子站应应答");
    assert!(matches!(reply, Frame::Fixed { .. }));
    assert_eq!(s.value(1), Some(PointValue::Bool(true)));
    assert!(s
        .drain_events()
        .contains(&StationEvent::ControlConfirmed { ioa: 1, on: false }));
}

#[test]
fn control_unknown_point_gets_negative_confirm() {
    let mut m = master();
    let mut s = slave(vec![]);

    step(&mut m, &mut s);
    assert!(m.control(99, true));

    for _ in 0..20 {
        step(&mut m, &mut s);
    }

    assert_eq!(s.stats().errors, 1, "子站应记录一次错误");
    assert!(m
        .drain_events()
        .iter()
        .any(|e| matches!(e, MasterEvent101::CommandFailed { .. })));
}

#[test]
fn clock_sync_confirms_and_stores_time() {
    let mut m = master();
    let mut s = slave(vec![]);

    step(&mut m, &mut s);
    let time = Cp56Time2a {
        milliseconds: 12345,
        minute: 30,
        hour: 12,
        day: 15,
        month: 6,
        year: 2026,
        invalid: false,
        summer_time: false,
    };
    assert!(m.sync_clock(time));

    for _ in 0..20 {
        step(&mut m, &mut s);
    }

    assert!(m.drain_events().contains(&MasterEvent101::ClockSynced));
    assert_eq!(s.last_clock_sync(), Some(12345));
}

#[test]
fn reset_process_confirms_and_emits_event() {
    let mut m = master();
    let mut s = slave(vec![]);

    step(&mut m, &mut s);

    let reset = Asdu {
        type_id: 105,
        sq: false,
        cause: 6,
        negative: false,
        test: false,
        originator: 0,
        address: config().address,
        objects: vec![InformationObject::ResetProcess { ioa: 0, qrp: 1 }],
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
        asdu: encode_asdu(&reset).expect("编码应成功"),
    };
    let reply = s.handle_frame(&frame).expect("子站应应答");
    assert!(matches!(reply, Frame::Fixed { .. }));
    assert!(s.drain_events().contains(&StationEvent::ProcessReset));
}

#[test]
fn duplicate_frame_executes_business_once() {
    let mut m = master();
    let mut s = slave(vec![(1, PointKind::SinglePoint)]);

    step(&mut m, &mut s);
    assert!(m.interrogate(20));
    let frame = m.next_frame().expect("主站应发出召唤帧");

    // 第一次投递:新帧,执行总召唤并应答。
    let first = s.handle_frame(&frame).expect("子站应应答");
    assert_eq!(s.stats().interrogations, 1);

    // 同一帧再投一次:判为重复帧,重发同一应答,不重复执行业务。
    let second = s.handle_frame(&frame).expect("重复帧应重发应答");
    assert_eq!(first, second);
    assert_eq!(s.stats().interrogations, 1, "重复帧不应重复执行总召唤");
}

#[test]
fn ack_timeout_retransmits_then_fails() {
    let mut m = master();
    let mut s = slave(vec![]);

    step(&mut m, &mut s);
    assert!(m.interrogate(20));
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

    // 达到上限后不再重发,链路复位并产生命令失败事件。
    assert!(m.tick(timeout).is_none());
    assert!(m
        .drain_events()
        .iter()
        .any(|e| matches!(e, MasterEvent101::CommandFailed { .. })));

    // 链路回到 Reset,下一帧是复位链路帧。
    match m.next_frame() {
        Some(Frame::Fixed { control, .. }) => {
            assert_eq!(ControlField::parse(control).function, 0);
        }
        other => panic!("应为复位链路帧,实际 {other:?}"),
    }
}

#[test]
fn common_address_mismatch_is_rejected() {
    let mut m = master();
    let mut s = slave(vec![(1, PointKind::SinglePoint)]);

    step(&mut m, &mut s);

    // 构造公共地址不匹配的召唤 ASDU。
    let wrong = Asdu {
        type_id: 100,
        sq: false,
        cause: 6,
        negative: false,
        test: false,
        originator: 0,
        address: AsduAddress {
            common_address: 99,
            ca_len: 1,
            ioa_len: 2,
        },
        objects: vec![InformationObject::InterrogationCommand { ioa: 0, qoi: 20 }],
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
        asdu: encode_asdu(&wrong).expect("编码应成功"),
    };
    let _ = s.handle_frame(&frame);
    assert_eq!(
        s.stats().interrogations, 0,
        "公共地址不匹配时不应执行总召唤"
    );
    assert_eq!(s.stats().errors, 1, "应记录一次错误");
    assert!(s
        .drain_events()
        .contains(&StationEvent::CommonAddressMismatch {
            expected: 1,
            actual: 99,
        }));
}
