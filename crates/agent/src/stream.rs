//! Agent side of a multiplexed stream.

use bytes::Bytes;
use croncave_proto::{Frame, FrameKind, StreamError};
use tokio::sync::mpsc;

pub enum Incoming {
    Data(Bytes),
    End,
    Reset,
}

pub struct AgentStream {
    pub id: u32,
    rx: mpsc::Receiver<Incoming>,
    out: mpsc::Sender<Frame>,
    read_done: bool,
}

impl AgentStream {
    pub fn new(id: u32, rx: mpsc::Receiver<Incoming>, out: mpsc::Sender<Frame>) -> Self {
        Self { id, rx, out, read_done: false }
    }

    /// The request body, up to `limit` bytes.
    pub async fn read_body(&mut self, limit: usize) -> Result<Vec<u8>, String> {
        let mut buf = Vec::new();
        while let Some(chunk) = self.recv().await? {
            if buf.len() + chunk.len() > limit {
                return Err("the request is too large".into());
            }
            buf.extend_from_slice(&chunk);
        }
        Ok(buf)
    }

    /// The next body chunk, `None` at the end.
    pub async fn recv(&mut self) -> Result<Option<Bytes>, String> {
        if self.read_done {
            return Ok(None);
        }
        match self.rx.recv().await {
            Some(Incoming::Data(b)) => Ok(Some(b)),
            Some(Incoming::End) => {
                self.read_done = true;
                Ok(None)
            }
            Some(Incoming::Reset) | None => {
                self.read_done = true;
                Err("the platform closed the stream".into())
            }
        }
    }

    pub async fn send(&self, data: Bytes) -> bool {
        for chunk in data.chunks(256 * 1024) {
            if self.out.send(Frame::new(FrameKind::Data, self.id, Bytes::copy_from_slice(chunk))).await.is_err() {
                return false;
            }
        }
        true
    }

    pub async fn send_json<T: serde::Serialize>(&self, value: &T) -> bool {
        self.send(Bytes::from(serde_json::to_vec(value).expect("answers serialize"))).await
    }

    pub async fn send_error(&self, error: impl Into<String>) -> bool {
        self.send_json(&StreamError { error: error.into() }).await
    }

    pub async fn end(&self) {
        let _ = self.out.send(Frame::new(FrameKind::End, self.id, Bytes::new())).await;
    }
}
