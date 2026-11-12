//! 非平衡链路状态机集成测试。
//!
//! 主站与子站两个状态机在内存里互相对发,不经过网络或串口。

use iec101_core::ft12::{ControlField, Frame};
use iec101_core::link::{LinkParams, LinkState, MasterEvent, MasterLink, SlaveLink};

const ADDRESS: u16 = 0x01;

fn params() -> LinkParams {
    LinkParams {
        address: ADDRESS,
        address_len: 1,
        max_retries: 3,
    }
}

fn master() -> MasterLink {
    MasterLink::new(params())
}

fn slave() -> SlaveLink {
    SlaveLink::new(params())
}

/// 主站发复位帧,把子站应答交给主站,完成一次链路复位。
fn sync(m: &mut MasterLink, s: &mut SlaveLink) {
    let reset = m.reset_link();
    match s.handle(&reset) {
        Some(reply) => assert_eq!(m.handle(&reply), MasterEvent::LinkOk),
        None => assert_eq!("子站应应答复位链路", "返回 None"),
    }
}

/// 子站处理一帧并返回应答,没有应答时让测试失败。
fn reply_of(s: &mut SlaveLink, frame: &Frame) -> Frame {
    match s.handle(frame) {
        Some(reply) => reply,
        None => {
            assert_eq!("子站应返回应答帧", "返回 None");
            Frame::Ack
        }
    }
}

/// 主站发帧、子站应答、主站处理,返回主站事件。
fn exchange(m: &mut MasterLink, s: &mut SlaveLink, frame: &Frame) -> Option<MasterEvent> {
    s.handle(frame).map(|reply| m.handle(&reply))
}

/// 翻转帧里的 FCB 位(bit5),构造下一帧用户数据。
fn flip_fcb(frame: &Frame) -> Frame {
    match frame {
        Frame::Variable {
            control,
            address,
            asdu,
        } => Frame::Variable {
            control: control ^ 0x20,
            address: *address,
            asdu: asdu.clone(),
        },
        _ => frame.clone(),
    }
}

/// 断言事件是用户数据,返回 ASDU、ACD、DFC。
fn data_of(ev: Option<MasterEvent>) -> Option<(Vec<u8>, bool, bool)> {
    match ev {
        Some(MasterEvent::Data { asdu, acd, dfc }) => Some((asdu, acd, dfc)),
        _ => None,
    }
}

/// 读取帧控制域的 FCB 位。
fn fcb_of(frame: &Frame) -> bool {
    let control = match frame {
        Frame::Variable { control, .. } => *control,
        Frame::Fixed { control, .. } => *control,
        Frame::Ack => unreachable!("FCB 只在固定帧和可变帧里"),
    };
    ControlField::parse(control).fcb
}

#[test]
fn reset_link_flow() {
    let mut m = master();
    let mut s = slave();
    assert_eq!(m.state(), LinkState::Reset);
    assert_eq!(s.state(), LinkState::Reset);

    let reset = m.reset_link();
    let reply = reply_of(&mut s, &reset);

    assert_eq!(m.handle(&reply), MasterEvent::LinkOk);
    assert_eq!(m.state(), LinkState::Available);
    assert_eq!(s.state(), LinkState::Available);
}

#[test]
fn secondary_data_polling_twice() {
    let mut m = master();
    let mut s = slave();
    sync(&mut m, &mut s);

    s.push_secondary(vec![0x01, 0x02]);
    s.push_secondary(vec![0x03, 0x04]);

    let first = m.poll();
    let data = data_of(exchange(&mut m, &mut s, &first));
    assert_eq!(data.map(|(asdu, _, _)| asdu), Some(vec![0x01, 0x02]));

    let second = m.poll();
    let data = data_of(exchange(&mut m, &mut s, &second));
    assert_eq!(data.map(|(asdu, _, _)| asdu), Some(vec![0x03, 0x04]));

    assert_eq!(s.pending(), (0, 0));
}

#[test]
fn primary_takes_priority_over_secondary() {
    let mut m = master();
    let mut s = slave();
    sync(&mut m, &mut s);

    s.push_primary(vec![0xAA]);
    s.push_secondary(vec![0xBB]);

    let first = m.poll();
    let data = data_of(exchange(&mut m, &mut s, &first));
    assert_eq!(data.map(|(asdu, _, _)| asdu), Some(vec![0xAA]));

    let second = m.poll();
    let data = data_of(exchange(&mut m, &mut s, &second));
    assert_eq!(data.map(|(asdu, _, _)| asdu), Some(vec![0xBB]));

    assert_eq!(s.pending(), (0, 0));
}

#[test]
fn acd_set_until_last_primary() {
    let mut m = master();
    let mut s = slave();
    sync(&mut m, &mut s);

    s.push_primary(vec![0x01]);
    s.push_primary(vec![0x02]);
    s.push_primary(vec![0x03]);

    let expected = [(0x01, true), (0x02, true), (0x03, false)];
    for (asdu_byte, want_acd) in expected {
        let frame = m.poll();
        let data = data_of(exchange(&mut m, &mut s, &frame));
        match data {
            Some((asdu, acd, _)) => {
                assert_eq!(asdu, vec![asdu_byte]);
                assert_eq!(acd, want_acd);
            }
            None => assert_eq!("期望用户数据", "没有数据"),
        }
    }
}

#[test]
fn duplicate_frame_is_replayed_not_reprocessed() {
    let mut m = master();
    let mut s = slave();
    sync(&mut m, &mut s);

    m.send_user_data(vec![0x10]);
    let frame = m.poll();
    assert!(matches!(frame, Frame::Variable { .. }));

    // 第一次发送被接受。
    let first = reply_of(&mut s, &frame);
    assert_eq!(
        m.handle(&first),
        MasterEvent::Ack {
            acd: false,
            dfc: false
        }
    );
    assert_eq!(s.command_count(), 1);

    // 同一 FCB 再发一次,判为重复帧,重发同一应答,不再入队。
    let duplicate = reply_of(&mut s, &frame);
    assert_eq!(first, duplicate);
    assert_eq!(s.command_count(), 1);

    // 翻转 FCB 后发送,应被接受。
    let next = flip_fcb(&frame);
    let third = reply_of(&mut s, &next);
    assert_eq!(s.command_count(), 2);
    assert_eq!(third, first);
}

#[test]
fn fcb_starts_at_one_after_reset() {
    let mut m = master();
    let mut s = slave();
    sync(&mut m, &mut s);

    m.send_user_data(vec![0x01]);
    let first = m.poll();
    assert_eq!(fcb_of(&first), true);

    // 确认第一帧后再发第二帧,FCB 翻转为 0。
    let reply = reply_of(&mut s, &first);
    assert!(matches!(m.handle(&reply), MasterEvent::Ack { .. }));
    m.send_user_data(vec![0x02]);
    let second = m.poll();
    assert_eq!(fcb_of(&second), false);
}

#[test]
fn slave_accepts_first_frame_after_reset() {
    let mut m = master();
    let mut s = slave();
    sync(&mut m, &mut s);

    m.send_user_data(vec![0x10]);
    let frame = m.poll();
    assert_eq!(fcb_of(&frame), true);

    // 复位后的第一帧必须被接受,不能判成重复帧。
    let reply = reply_of(&mut s, &frame);
    assert_eq!(s.command_count(), 1);
    assert_eq!(
        m.handle(&reply),
        MasterEvent::Ack {
            acd: false,
            dfc: false
        }
    );
}

#[test]
fn fcb_restarts_at_one_after_second_reset() {
    let mut m = master();
    let mut s = slave();
    sync(&mut m, &mut s);

    // 先走两帧,FCB 依次是 1、0。
    let mut expect = [true, false].into_iter();
    for asdu in [0x01, 0x02] {
        m.send_user_data(vec![asdu]);
        let frame = m.poll();
        assert_eq!(fcb_of(&frame), expect.next().unwrap());
        let reply = reply_of(&mut s, &frame);
        assert!(matches!(m.handle(&reply), MasterEvent::Ack { .. }));
    }

    // 再次复位后,第一帧的 FCB 重新从 1 开始。
    let reset = m.reset_link();
    let reply = reply_of(&mut s, &reset);
    assert_eq!(
        m.handle(&reply),
        MasterEvent::Ack {
            acd: false,
            dfc: false
        }
    );

    m.send_user_data(vec![0x03]);
    let frame = m.poll();
    assert_eq!(fcb_of(&frame), true);
}

#[test]
fn link_busy_state_then_status_poll() {
    let mut m = master();
    let mut s = slave();
    sync(&mut m, &mut s);

    let busy = Frame::Fixed {
        control: ControlField {
            dir: false,
            prm: false,
            fcb: false,
            fcv: false,
            acd: false,
            dfc: true,
            function: 11,
        }
        .encode(),
        address: ADDRESS,
    };
    assert_eq!(m.handle(&busy), MasterEvent::LinkBusy);
    assert_eq!(m.state(), LinkState::Busy);

    // 链路忙时 poll 仍能产生请求帧,这里发请求链路状态。
    let frame = m.poll();
    match &frame {
        Frame::Fixed { control, .. } => {
            let cf = ControlField::parse(*control);
            assert!(cf.prm);
            assert_eq!(cf.function, 9);
        }
        other => assert_eq!(format!("{:?}", other), "Fixed"),
    }
}

#[test]
fn timeout_retransmits_then_resets() {
    let mut m = master();
    let mut s = slave();
    sync(&mut m, &mut s);

    m.send_user_data(vec![0x10]);
    let frame = m.poll();
    assert!(matches!(frame, Frame::Variable { .. }));

    // 连续超时 max_retries 次,每次都重发同一帧,内容不变。
    for _ in 0..params().max_retries {
        match m.on_timeout() {
            Some(resent) => assert_eq!(resent, frame),
            None => assert_eq!("超时应重发", "返回 None"),
        }
    }

    // 超过 max_retries 后不再重发,链路回到 Reset。
    assert_eq!(m.on_timeout(), None);
    assert_eq!(m.state(), LinkState::Reset);
}

#[test]
fn unknown_function_returns_nak() {
    let mut s = slave();
    // 0x45:PRM=1, FCV=0, 功能码 5,未识别。
    let unknown = Frame::Fixed {
        control: 0x45,
        address: ADDRESS,
    };
    let reply = reply_of(&mut s, &unknown);
    match reply {
        Frame::Fixed { control, .. } => {
            let cf = ControlField::parse(control);
            assert!(!cf.prm);
            assert_eq!(cf.function, 1);
        }
        other => assert_eq!(format!("{:?}", other), "Fixed"),
    }
}

#[test]
fn fifty_rounds_polling_keeps_link_available() {
    let mut m = master();
    let mut s = slave();
    sync(&mut m, &mut s);

    for i in 0..5u8 {
        s.push_primary(vec![0x10 + i]);
        s.push_secondary(vec![0x20 + i]);
    }

    for _ in 0..50 {
        let frame = m.poll();
        if let Some(reply) = s.handle(&frame) {
            let _ = m.handle(&reply);
        }
    }

    assert_eq!(s.pending(), (0, 0));
    assert_eq!(m.state(), LinkState::Available);
}
