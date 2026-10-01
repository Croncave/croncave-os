//! Events wait on disk until the relay acknowledges them, so a reconnect loses nothing.

use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

use croncave_proto::{AgentEvent, SeqEvent};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub struct Outbox {
    dir: PathBuf,
    inner: Mutex<Inner>,
    notify: tokio::sync::Notify,
}

struct Inner {
    epoch: Uuid,
    next_seq: u64,
    /// Highest sequence number the relay has stored.
    acked: u64,
    /// Highest sequence number sent on the current connection.
    sent: u64,
    pending: Vec<SeqEvent>,
    file: std::fs::File,
}

#[derive(Serialize, Deserialize)]
struct State {
    epoch: Uuid,
    acked: u64,
}

impl Outbox {
    pub fn open(dir: PathBuf) -> anyhow::Result<Self> {
        std::fs::create_dir_all(&dir)?;
        let state: State = std::fs::read(dir.join("outbox.state"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or(State { epoch: Uuid::new_v4(), acked: 0 });
        let pending: Vec<SeqEvent> = std::fs::read_to_string(dir.join("outbox.jsonl"))
            .unwrap_or_default()
            .lines()
            .filter_map(|l| serde_json::from_str::<SeqEvent>(l).ok())
            .filter(|e| e.seq > state.acked)
            .collect();
        let next_seq = pending.iter().map(|e| e.seq).max().unwrap_or(state.acked) + 1;
        let file = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("outbox.jsonl"))?;
        let outbox = Self {
            dir,
            inner: Mutex::new(Inner {
                epoch: state.epoch,
                next_seq,
                acked: state.acked,
                sent: state.acked,
                pending,
                file,
            }),
            notify: tokio::sync::Notify::new(),
        };
        outbox.write_state();
        Ok(outbox)
    }

    pub fn epoch(&self) -> Uuid {
        self.inner.lock().expect("outbox").epoch
    }

    pub fn push(&self, event: AgentEvent) -> std::io::Result<u64> {
        let mut inner = self.inner.lock().expect("outbox");
        let seq = inner.next_seq;
        inner.next_seq += 1;
        let ev = SeqEvent { seq, at_ms: chrono::Utc::now().timestamp_millis(), event };
        let line = serde_json::to_string(&ev).expect("events serialize");
        writeln!(inner.file, "{line}")?;
        inner.pending.push(ev);
        drop(inner);
        self.notify.notify_one();
        Ok(seq)
    }

    /// Events not yet sent on this connection (at most `max`), marking them sent.
    pub fn take_unsent(&self, max: usize) -> Vec<SeqEvent> {
        let mut inner = self.inner.lock().expect("outbox");
        let sent = inner.sent;
        let batch: Vec<SeqEvent> = inner.pending.iter().filter(|e| e.seq > sent).take(max).cloned().collect();
        if let Some(last) = batch.last() {
            inner.sent = last.seq;
        }
        batch
    }

    /// A new connection: everything unacknowledged must be sent again.
    pub fn rewind(&self) {
        let mut inner = self.inner.lock().expect("outbox");
        inner.sent = inner.acked;
        drop(inner);
        self.notify.notify_one();
    }

    pub fn ack(&self, upto: u64) {
        let mut inner = self.inner.lock().expect("outbox");
        if upto <= inner.acked {
            return;
        }
        inner.acked = upto;
        inner.pending.retain(|e| e.seq > upto);
        let compact = inner.pending.is_empty();
        drop(inner);
        self.write_state();
        if compact {
            self.flush_to_disk();
        }
    }

    /// Rewrite the file with only unacknowledged events.
    pub fn flush_to_disk(&self) {
        let mut inner = self.inner.lock().expect("outbox");
        let body: String =
            inner.pending.iter().map(|e| serde_json::to_string(e).expect("events serialize") + "\n").collect();
        let path = self.dir.join("outbox.jsonl");
        let tmp = self.dir.join("outbox.jsonl.tmp");
        if std::fs::write(&tmp, body).is_ok()
            && std::fs::rename(&tmp, &path).is_ok()
            && let Ok(f) = std::fs::OpenOptions::new().append(true).open(&path)
        {
            inner.file = f;
        }
    }

    pub async fn wait(&self) {
        self.notify.notified().await
    }

    pub fn pending_len(&self) -> usize {
        self.inner.lock().expect("outbox").pending.len()
    }

    fn write_state(&self) {
        let inner = self.inner.lock().expect("outbox");
        let state = State { epoch: inner.epoch, acked: inner.acked };
        let _ = std::fs::write(self.dir.join("outbox.state"), serde_json::to_vec(&state).expect("state serializes"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev() -> AgentEvent {
        AgentEvent::RunStarted { run_id: Uuid::nil() }
    }

    #[test]
    fn unacknowledged_events_survive_a_restart_and_resend() {
        let dir = tempfile::tempdir().unwrap();
        let ob = Outbox::open(dir.path().to_path_buf()).unwrap();
        let epoch = ob.epoch();
        for _ in 0..3 {
            ob.push(ev()).unwrap();
        }
        assert_eq!(ob.take_unsent(10).len(), 3);
        assert!(ob.take_unsent(10).is_empty());
        ob.ack(1);
        drop(ob);

        let ob = Outbox::open(dir.path().to_path_buf()).unwrap();
        assert_eq!(ob.epoch(), epoch);
        let again: Vec<u64> = ob.take_unsent(10).iter().map(|e| e.seq).collect();
        assert_eq!(again, vec![2, 3]);
        assert_eq!(ob.push(ev()).unwrap(), 4, "numbering continues after a restart");
    }

    #[test]
    fn rewind_resends_after_reconnect_and_ack_compacts() {
        let dir = tempfile::tempdir().unwrap();
        let ob = Outbox::open(dir.path().to_path_buf()).unwrap();
        ob.push(ev()).unwrap();
        ob.push(ev()).unwrap();
        ob.take_unsent(10);
        ob.rewind();
        assert_eq!(ob.take_unsent(10).len(), 2);
        ob.ack(2);
        assert_eq!(ob.pending_len(), 0);
        let file = std::fs::read_to_string(dir.path().join("outbox.jsonl")).unwrap();
        assert!(file.is_empty());
        assert_eq!(ob.push(ev()).unwrap(), 3);
    }
}
