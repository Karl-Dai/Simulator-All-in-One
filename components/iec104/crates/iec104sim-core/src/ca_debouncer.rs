//! 未知 CA debouncer:广播召唤期间收集陌生 CA,3 秒安静期后一次性 flush。
//!
//! 协议层不直接持有 Tauri AppHandle —— 通过 `flush_tx` 把"该 flush 这些 CA"
//! 的事件抛给上层(commands 层),由上层去 emit Tauri 事件。

use std::collections::HashSet;
use std::time::Duration;
use tokio::sync::mpsc;

/// 单次 flush 抛出的 CA 集合(已去重)。
#[derive(Debug, Clone)]
pub struct CaFlushEvent {
    pub new_cas: Vec<u16>,
}

/// 由 commands 层持有的入口:把陌生 CA 喂进来。
#[derive(Clone)]
pub struct CaInbox {
    tx: mpsc::UnboundedSender<u16>,
}

impl CaInbox {
    pub fn push(&self, ca: u16) {
        let _ = self.tx.send(ca);
    }
}

/// 启动 debouncer:返回一个 `CaInbox`(用于喂 CA)+ `flush_rx`(用于接 flush 事件)+ 后台 JoinHandle。
///
/// `settle` 是安静期长度;每次新 CA 出现重置定时器。Channel 关闭(所有 `CaInbox` 被 drop)且
/// `state` 非空时,做最后一次 flush 然后退出 —— 这是断连前 "强制 flush" 的自然路径。
pub fn spawn(
    settle: Duration,
) -> (CaInbox, mpsc::UnboundedReceiver<CaFlushEvent>, tokio::task::JoinHandle<()>) {
    let (in_tx, mut in_rx) = mpsc::unbounded_channel::<u16>();
    let (out_tx, out_rx) = mpsc::unbounded_channel::<CaFlushEvent>();

    let handle = tokio::spawn(async move {
        let mut state: HashSet<u16> = HashSet::new();
        let mut deadline: Option<tokio::time::Instant> = None;
        loop {
            let sleep = match deadline {
                Some(d) => tokio::time::sleep_until(d),
                None => tokio::time::sleep(Duration::from_secs(3600)),
            };
            tokio::pin!(sleep);

            tokio::select! {
                maybe_ca = in_rx.recv() => {
                    match maybe_ca {
                        Some(ca) => {
                            state.insert(ca);
                            deadline = Some(tokio::time::Instant::now() + settle);
                        }
                        None => {
                            if !state.is_empty() {
                                let mut cas: Vec<u16> = state.drain().collect();
                                cas.sort();
                                let _ = out_tx.send(CaFlushEvent { new_cas: cas });
                            }
                            return;
                        }
                    }
                }
                _ = &mut sleep, if deadline.is_some() => {
                    if !state.is_empty() {
                        let mut cas: Vec<u16> = state.drain().collect();
                        cas.sort();
                        let _ = out_tx.send(CaFlushEvent { new_cas: cas });
                    }
                    deadline = None;
                }
            }
        }
    });

    (CaInbox { tx: in_tx }, out_rx, handle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test(start_paused = true)]
    async fn flushes_after_quiet_period() {
        let (inbox, mut rx, _handle) = spawn(Duration::from_secs(3));
        inbox.push(11);
        inbox.push(12);
        inbox.push(13);
        tokio::time::sleep(Duration::from_millis(2900)).await;
        assert!(rx.try_recv().is_err());
        tokio::time::sleep(Duration::from_millis(200)).await;
        let ev = rx.recv().await.expect("expected flush");
        assert_eq!(ev.new_cas, vec![11, 12, 13]);
    }

    #[tokio::test(start_paused = true)]
    async fn resets_deadline_on_new_ca() {
        let (inbox, mut rx, _handle) = spawn(Duration::from_secs(3));
        inbox.push(1);
        tokio::time::sleep(Duration::from_secs(2)).await;
        inbox.push(2);
        tokio::time::sleep(Duration::from_millis(2900)).await;
        assert!(rx.try_recv().is_err(), "must not flush before reset deadline");
        tokio::time::sleep(Duration::from_millis(200)).await;
        let ev = rx.recv().await.unwrap();
        assert_eq!(ev.new_cas, vec![1, 2]);
    }

    #[tokio::test(start_paused = true)]
    async fn dedupes_same_ca() {
        let (inbox, mut rx, _handle) = spawn(Duration::from_secs(1));
        for _ in 0..5 { inbox.push(7); }
        tokio::time::sleep(Duration::from_millis(1100)).await;
        let ev = rx.recv().await.unwrap();
        assert_eq!(ev.new_cas, vec![7]);
    }

    #[tokio::test(start_paused = true)]
    async fn forces_flush_on_inbox_drop() {
        let (inbox, mut rx, handle) = spawn(Duration::from_secs(60));
        inbox.push(42);
        drop(inbox);
        let _ = tokio::time::timeout(Duration::from_secs(5), handle).await;
        let ev = rx.recv().await.unwrap();
        assert_eq!(ev.new_cas, vec![42]);
    }

    #[tokio::test(start_paused = true)]
    async fn no_flush_when_state_empty() {
        let (_inbox, mut rx, _handle) = spawn(Duration::from_millis(100));
        tokio::time::sleep(Duration::from_millis(500)).await;
        assert!(rx.try_recv().is_err());
    }
}
