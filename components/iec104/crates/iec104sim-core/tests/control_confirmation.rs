//! Confirmed single-frame controls use real TCP replies, not send completion.
use iec104sim_core::log_entry::FrameLabel;
use iec104sim_core::master::{MasterConfig, MasterConnection};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

struct Peer {
    stream: TcpStream,
    send_sequence: u16,
    received_sequence: u16,
}

async fn read_frame(stream: &mut TcpStream) -> Vec<u8> {
    let mut header = [0; 2];
    stream.read_exact(&mut header).await.unwrap();
    assert_eq!(header[0], 0x68);
    let mut frame = vec![0; header[1] as usize + 2];
    frame[..2].copy_from_slice(&header);
    stream.read_exact(&mut frame[2..]).await.unwrap();
    frame
}

impl Peer {
    async fn command(&mut self) -> Vec<u8> {
        loop {
            let frame = read_frame(&mut self.stream).await;
            if frame[2] & 1 == 0 {
                self.received_sequence = u16::from_le_bytes([frame[2], frame[3]]) >> 1;
                return frame;
            }
            if frame[2] == 0x43 {
                self.stream.write_all(&[0x68, 4, 0x83, 0, 0, 0]).await.unwrap();
            }
        }
    }

    async fn response(&mut self, mut frame: Vec<u8>, cause: u8) {
        frame[2..4].copy_from_slice(&(self.send_sequence << 1).to_le_bytes());
        frame[4..6].copy_from_slice(&((self.received_sequence + 1) << 1).to_le_bytes());
        frame[8] = cause;
        self.send_sequence += 1;
        self.stream.write_all(&frame).await.unwrap();
    }
}

async fn connected() -> (MasterConnection, Peer) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let accept = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        stream.set_nodelay(true).unwrap();
        assert_eq!(read_frame(&mut stream).await, vec![0x68, 4, 7, 0, 0, 0]);
        stream.write_all(&[0x68, 4, 0x0b, 0, 0, 0]).await.unwrap();
        Peer { stream, send_sequence: 0, received_sequence: 0 }
    });
    let mut master = MasterConnection::new(MasterConfig {
        target_address: "127.0.0.1".to_string(), port,
        interrogate_period_s: 0, counter_interrogate_period_s: 0,
        t1: 15, t2: 10, t3: 20,
        ..Default::default()
    });
    master.connect().await.unwrap();
    (master, accept.await.unwrap())
}

fn float_command(selected: bool, cause: u8) -> Vec<u8> {
    let mut frame = vec![0x68, 18, 0, 0, 0, 0, 50, 1, cause, 0, 2, 0, 1, 98, 0];
    frame.extend_from_slice(&44.25_f32.to_le_bytes());
    frame.push(if selected { 0x80 } else { 0 });
    frame
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn direct_execute_waits_for_delayed_ack_and_reports_elapsed_time() {
    let (mut master, mut peer) = connected().await;
    let frame = float_command(false, 6);
    let (result, _) = tokio::join!(
        master.send_control_frame_with_confirmation(frame.clone(), "test", FrameLabel::SetpointFloat, 2, None),
        async {
            let request = peer.command().await;
            assert_eq!(&request[6..], &frame[6..]);
            tokio::time::sleep(Duration::from_millis(220)).await;
            peer.response(request, 7).await;
        }
    );
    let result = result.unwrap();
    assert_eq!(result.steps.iter().map(|s| s.action.as_str()).collect::<Vec<_>>(), ["execute_sent", "execute_confirmed"]);
    assert!(result.duration_ms >= 200, "must include reply latency: {}", result.duration_ms);
    master.disconnect().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn select_only_confirms_without_sending_execute() {
    let (mut master, mut peer) = connected().await;
    let (result, _) = tokio::join!(
        master.send_control_frame_with_confirmation(float_command(true, 6), "test", FrameLabel::SetpointFloat, 2, None),
        async {
            let request = peer.command().await;
            assert_eq!(request[19] & 0x80, 0x80);
            peer.response(request, 7).await;
            assert!(tokio::time::timeout(Duration::from_millis(200), peer.command()).await.is_err(), "select-only sent a second command");
        }
    );
    assert_eq!(result.unwrap().steps.last().unwrap().action, "select_confirmed");
    master.disconnect().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn negative_and_protocol_error_responses_are_failures() {
    for cause in [0x47, 44, 45, 46, 47] {
        let (mut master, mut peer) = connected().await;
        let (result, _) = tokio::join!(
            master.send_control_frame_with_confirmation(float_command(false, 6), "test", FrameLabel::SetpointFloat, 2, None),
            async {
                let request = peer.command().await;
                peer.response(request, cause).await;
            }
        );
        assert!(result.unwrap_err().to_string().contains("拒绝"), "cause {cause}");
        master.disconnect().await.unwrap();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unrelated_ca_type_ioa_value_and_select_replies_cannot_confirm_execute() {
    let (mut master, mut peer) = connected().await;
    let (result, _) = tokio::join!(
        master.send_control_frame_with_confirmation(float_command(false, 6), "test", FrameLabel::SetpointFloat, 2, None),
        async {
            let request = peer.command().await;
            // Same IOA alone is insufficient: exercise each identity dimension.
            for index in [10, 6, 12, 15, 19, 9] {
                let mut unrelated = request.clone();
                unrelated[index] ^= if index == 19 { 0x80 } else { 1 };
                peer.response(unrelated, 7).await;
            }
            peer.response(request.clone(), 7 | 0x80).await; // Test flag differs.
            peer.response(request.clone(), 10).await; // ACTTERM is not ACTCON.
            tokio::time::sleep(Duration::from_millis(220)).await;
            peer.response(request, 7).await;
        }
    );
    assert!(result.unwrap().duration_ms >= 200);
    master.disconnect().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn irrelevant_replies_do_not_extend_five_second_deadline() {
    let (mut master, mut peer) = connected().await;
    let start = Instant::now();
    let (result, _) = tokio::join!(
        master.send_control_frame_with_confirmation(float_command(false, 6), "test", FrameLabel::SetpointFloat, 2, None),
        async {
            let mut unrelated = peer.command().await;
            unrelated[10] = 99;
            for _ in 0..27 {
                peer.response(unrelated.clone(), 7).await;
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        }
    );
    assert!(result.unwrap_err().to_string().contains("超时"));
    assert!(start.elapsed() < Duration::from_secs(7));
    master.disconnect().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_disconnect_fails_before_confirmation_timeout() {
    let (mut master, mut peer) = connected().await;
    let start = Instant::now();
    let (result, _) = tokio::join!(
        master.send_control_frame_with_confirmation(float_command(false, 6), "test", FrameLabel::SetpointFloat, 2, None),
        async move { peer.command().await; drop(peer); }
    );
    assert!(result.unwrap_err().to_string().contains("断开"));
    assert!(start.elapsed() < Duration::from_secs(2));
    // The peer already closed the socket; explicit teardown reports that state.
    assert!(matches!(master.disconnect().await, Err(iec104sim_core::master::MasterError::NotConnected)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_control_is_rejected_without_an_extra_frame() {
    let (mut master, mut peer) = connected().await;
    let (first, second) = tokio::join!(
        master.send_control_frame_with_confirmation(float_command(false, 6), "first", FrameLabel::SetpointFloat, 2, None),
        async {
            let request = peer.command().await;
            let error = master.send_control_frame_with_confirmation(float_command(false, 6), "second", FrameLabel::SetpointFloat, 2, None).await.unwrap_err();
            assert!(error.to_string().contains("已有控制"));
            peer.response(request, 7).await;
            assert!(tokio::time::timeout(Duration::from_millis(150), peer.command()).await.is_err());
        }
    );
    first.unwrap();
    let _ = second;
    master.disconnect().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn deactivation_waits_for_cot9_and_custom_cause_reports_sent_only() {
    let (mut master, mut peer) = connected().await;
    let (result, _) = tokio::join!(
        master.send_control_frame_with_confirmation(float_command(false, 8), "stop", FrameLabel::SetpointFloat, 2, None),
        async { let request = peer.command().await; peer.response(request, 9).await; }
    );
    assert_eq!(result.unwrap().steps.last().unwrap().action, "deactivate_confirmed");
    let result = master.send_control_frame_with_confirmation(float_command(false, 3), "custom", FrameLabel::SetpointFloat, 2, None).await.unwrap();
    assert_eq!(result.steps.len(), 1);
    assert_eq!(result.steps[0].action, "execute_sent");
    assert_eq!(peer.command().await[8], 3);
    master.disconnect().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sbo_ignores_duplicate_select_confirmation_during_execute() {
    let (mut master, mut peer) = connected().await;
    let (result, _) = tokio::join!(
        master.send_control_with_sbo(float_command(true, 6), float_command(false, 6), 25089, "sbo", FrameLabel::SetpointFloat, 2),
        async {
            let select = peer.command().await;
            peer.response(select.clone(), 7).await;
            let execute = peer.command().await;
            assert_eq!(execute[19] & 0x80, 0);
            peer.response(select, 7).await;
            tokio::time::sleep(Duration::from_millis(220)).await;
            peer.response(execute, 7).await;
        }
    );
    let result = result.unwrap();
    assert_eq!(result.steps.len(), 4);
    assert_eq!(result.steps.last().unwrap().action, "execute_confirmed");
    assert!(result.duration_ms >= 200);
    master.disconnect().await.unwrap();
}
