use std::sync::Arc;

use bytes::Bytes;
use croncave_proto::{Frame, FrameKind, StreamError};
use tokio::sync::mpsc;

use crate::{Conn, RelayError};

#[derive(Debug)]
pub enum StreamItem {
    Data(Bytes),
    End,
    Reset(String),
}

/// One stream to a computer, opened by the control plane through the relay.
pub struct RelayStream {
    id: u32,
    conn: Arc<Conn>,
    rx: mpsc::Receiver<StreamItem>,
    write_closed: bool,
    read_closed: bool,
}

impl RelayStream {
    pub(crate) fn new(id: u32, conn: Arc<Conn>, rx: mpsc::Receiver<StreamItem>) -> Self {
        Self { id, conn, rx, write_closed: false, read_closed: false }
    }

    pub async fn send(&mut self, data: Bytes) -> Result<(), RelayError> {
        // Keep frames well under the WebSocket message limit.
        for chunk in data.chunks(256 * 1024) {
            self.conn
                .out
                .send(Frame::new(FrameKind::Data, self.id, Bytes::copy_from_slice(chunk)))
                .await
                .map_err(|_| RelayError::Closed)?;
        }
        Ok(())
    }

    /// Nothing more to send.
    pub async fn end(&mut self) -> Result<(), RelayError> {
        if !self.write_closed {
            self.write_closed = true;
            self.conn
                .out
                .send(Frame::new(FrameKind::End, self.id, Bytes::new()))
                .await
                .map_err(|_| RelayError::Closed)?;
        }
        Ok(())
    }

    /// The next chunk, or `None` once the agent has finished sending.
    pub async fn recv(&mut self) -> Option<Result<Bytes, RelayError>> {
        if self.read_closed {
            return None;
        }
        match self.rx.recv().await {
            Some(StreamItem::Data(b)) => Some(Ok(b)),
            Some(StreamItem::End) => {
                self.read_closed = true;
                None
            }
            Some(StreamItem::Reset(reason)) => {
                self.read_closed = true;
                self.write_closed = true;
                Some(Err(RelayError::Reset(reason)))
            }
            None => {
                self.read_closed = true;
                Some(Err(RelayError::Closed))
            }
        }
    }

    /// Read one frame as JSON. A `{"error": ...}` answer becomes [`RelayError::Reset`].
    pub async fn recv_json<T: serde::de::DeserializeOwned>(&mut self) -> Result<T, RelayError> {
        let raw = match self.recv().await {
            Some(r) => r?,
            None => return Err(RelayError::Protocol("stream ended before its answer".into())),
        };
        if let Ok(err) = serde_json::from_slice::<StreamError>(&raw) {
            return Err(RelayError::Reset(err.error));
        }
        serde_json::from_slice(&raw).map_err(|e| RelayError::Protocol(e.to_string()))
    }

    /// Everything left on the stream.
    pub async fn read_to_end(&mut self) -> Result<Vec<u8>, RelayError> {
        let mut out = Vec::new();
        while let Some(chunk) = self.recv().await {
            out.extend_from_slice(&chunk?);
        }
        Ok(out)
    }
}

impl Drop for RelayStream {
    fn drop(&mut self) {
        self.conn.streams.lock().expect("stream map").remove(&self.id);
        if !(self.read_closed && self.write_closed) {
            let _ = self.conn.out.try_send(Frame::new(
                FrameKind::Reset,
                self.id,
                Bytes::from_static(b"closed by the platform"),
            ));
        }
    }
}
