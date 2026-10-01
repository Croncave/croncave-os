//! The relay: holds the one outgoing connection each computer's agent opens, and carries
//! every stream to and from it.
//!
//! The relay knows nothing about accounts, apps or providers. It authenticates agents with
//! short-lived credentials, keeps a routing table from computer to connection, opens
//! streams for the control plane, and hands agent events to [`RelayHooks`], acknowledging
//! them only after the hooks have stored them.

mod credential;
mod stream;

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use bytes::Bytes;
use croncave_proto::{AgentToRelay, Frame, FrameKind, Hello, RelayToAgent, SdkCall, SeqEvent, StreamOpen};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::{Mutex, mpsc};
use uuid::Uuid;

pub use credential::Credentials;
pub use stream::{RelayStream, StreamItem};

/// What the relay needs from the control plane.
#[async_trait]
pub trait RelayHooks: Send + Sync + 'static {
    /// Trade a one-time bootstrap token for the computer it was issued to.
    async fn redeem_bootstrap(&self, token: &str) -> Option<Uuid>;
    /// Whether a computer with a genuine credential may still connect (it may have been
    /// deleted, or belong to another deployment sharing the signing secret).
    async fn authorize(&self, computer: Uuid) -> bool;
    async fn connected(&self, computer: Uuid, hello: &Hello);
    async fn disconnected(&self, computer: Uuid);
    /// Store events durably. The relay acknowledges them to the agent only on `Ok`.
    async fn events(&self, computer: Uuid, events: Vec<SeqEvent>) -> anyhow::Result<()>;
    async fn sdk_call(&self, computer: Uuid, call: SdkCall) -> Result<Value, String>;
}

#[derive(Debug, thiserror::Error)]
pub enum RelayError {
    #[error("the computer isn't connected")]
    NotConnected,
    #[error("the connection to the computer closed")]
    Closed,
    #[error("the computer reported: {0}")]
    Reset(String),
    #[error("the computer sent something unexpected: {0}")]
    Protocol(String),
    #[error("the computer didn't answer in time")]
    Timeout,
}

#[derive(Clone)]
pub struct Relay {
    inner: Arc<Inner>,
}

struct Inner {
    hooks: Arc<dyn RelayHooks>,
    credentials: Credentials,
    /// The routing table: computer to the connection this node holds for it.
    conns: Mutex<HashMap<Uuid, Arc<Conn>>>,
    next_conn: AtomicU64,
    heartbeat: Duration,
}

pub(crate) struct Conn {
    id: u64,
    pub(crate) out: mpsc::Sender<Frame>,
    pub(crate) streams: std::sync::Mutex<HashMap<u32, mpsc::Sender<StreamItem>>>,
    next_stream: AtomicU32,
    last_pong: AtomicU64,
}

impl Relay {
    pub fn new(hooks: Arc<dyn RelayHooks>, secret: &[u8], credential_ttl: Duration) -> Self {
        Self {
            inner: Arc::new(Inner {
                hooks,
                credentials: Credentials::new(secret, credential_ttl),
                conns: Mutex::new(HashMap::new()),
                next_conn: AtomicU64::new(1),
                heartbeat: Duration::from_secs(10),
            }),
        }
    }

    /// Routes an agent uses: `POST /relay/token` and `GET /relay/connect`.
    pub fn router(&self) -> Router {
        Router::new().route("/relay/token", post(token)).route("/relay/connect", get(connect)).with_state(self.clone())
    }

    pub async fn is_connected(&self, computer: Uuid) -> bool {
        self.inner.conns.lock().await.contains_key(&computer)
    }

    pub async fn connected_computers(&self) -> Vec<Uuid> {
        self.inner.conns.lock().await.keys().copied().collect()
    }

    /// Send a control message to a computer.
    pub async fn send(&self, computer: Uuid, msg: &RelayToAgent) -> Result<(), RelayError> {
        let conn = self.conn(computer).await?;
        conn.out.send(Frame::control(msg)).await.map_err(|_| RelayError::Closed)
    }

    /// Open a stream to a computer.
    pub async fn open(&self, computer: Uuid, open: &StreamOpen) -> Result<RelayStream, RelayError> {
        let conn = self.conn(computer).await?;
        // Relay-opened streams use odd ids; 0 is control.
        let id = conn.next_stream.fetch_add(2, Ordering::Relaxed);
        let (tx, rx) = mpsc::channel(64);
        conn.streams.lock().expect("stream map").insert(id, tx);
        let payload = serde_json::to_vec(open).expect("stream open serializes");
        conn.out.send(Frame::new(FrameKind::Open, id, payload)).await.map_err(|_| RelayError::Closed)?;
        Ok(RelayStream::new(id, conn, rx))
    }

    /// Open a stream, send `body`, and read the JSON head the agent answers with.
    pub async fn request<T: serde::de::DeserializeOwned>(
        &self,
        computer: Uuid,
        open: &StreamOpen,
        body: Option<Bytes>,
    ) -> Result<(T, RelayStream), RelayError> {
        let mut s = self.open(computer, open).await?;
        if let Some(b) = body {
            s.send(b).await?;
        }
        s.end().await?;
        let head = tokio::time::timeout(Duration::from_secs(60), s.recv_json::<T>())
            .await
            .map_err(|_| RelayError::Timeout)??;
        Ok((head, s))
    }

    /// Close a computer's connection (it is going to sleep or being deleted).
    pub async fn disconnect(&self, computer: Uuid) {
        let conn = self.inner.conns.lock().await.remove(&computer);
        if let Some(conn) = conn {
            conn.reset_all("the computer went to sleep");
            self.inner.hooks.disconnected(computer).await;
        }
    }

    async fn conn(&self, computer: Uuid) -> Result<Arc<Conn>, RelayError> {
        self.inner.conns.lock().await.get(&computer).cloned().ok_or(RelayError::NotConnected)
    }
}

impl Conn {
    fn reset_all(&self, reason: &str) {
        let streams = std::mem::take(&mut *self.streams.lock().expect("stream map"));
        for (_, tx) in streams {
            let _ = tx.try_send(StreamItem::Reset(reason.to_string()));
        }
    }
}

#[derive(Deserialize)]
struct TokenRequest {
    bootstrap: String,
}

#[derive(Serialize, Deserialize)]
pub struct TokenResponse {
    pub credential: String,
    pub expires_at: i64,
}

async fn token(State(relay): State<Relay>, Json(req): Json<TokenRequest>) -> Response {
    match relay.inner.hooks.redeem_bootstrap(&req.bootstrap).await {
        Some(computer) => {
            let (credential, expires_at) = relay.inner.credentials.issue(computer);
            Json(TokenResponse { credential, expires_at }).into_response()
        }
        None => (StatusCode::UNAUTHORIZED, "bootstrap token is invalid or already used").into_response(),
    }
}

async fn connect(State(relay): State<Relay>, headers: HeaderMap, ws: WebSocketUpgrade) -> Response {
    let bearer = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or_default();
    // Authorization happens before any stream can open.
    let Some(computer) = relay.inner.credentials.verify(bearer) else {
        return (StatusCode::UNAUTHORIZED, "credential is invalid or expired").into_response();
    };
    if !relay.inner.hooks.authorize(computer).await {
        return (StatusCode::UNAUTHORIZED, "this computer is not known here").into_response();
    }
    ws.max_message_size(16 * 1024 * 1024).on_upgrade(move |socket| async move { relay.serve(computer, socket).await })
}

impl Relay {
    async fn serve(self, computer: Uuid, socket: WebSocket) {
        let (mut sink, mut source) = socket.split();
        let (out_tx, mut out_rx) = mpsc::channel::<Frame>(256);
        let conn = Arc::new(Conn {
            id: self.inner.next_conn.fetch_add(1, Ordering::Relaxed),
            out: out_tx.clone(),
            streams: std::sync::Mutex::new(HashMap::new()),
            next_stream: AtomicU32::new(1),
            last_pong: AtomicU64::new(now_secs()),
        });

        let writer = tokio::spawn(async move {
            while let Some(frame) = out_rx.recv().await {
                if sink.send(Message::Binary(frame.encode())).await.is_err() {
                    break;
                }
            }
            let _ = sink.close().await;
        });

        // The first message must be Hello.
        let hello = match tokio::time::timeout(Duration::from_secs(10), source.next()).await {
            Ok(Some(Ok(Message::Binary(raw)))) => match Frame::decode(raw) {
                Ok(f) if f.kind == FrameKind::Control => match serde_json::from_slice(&f.payload) {
                    Ok(AgentToRelay::Hello(h)) if h.computer_id == computer => Some(h),
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        };
        let Some(hello) = hello else {
            tracing::warn!(%computer, "agent did not say hello; closing");
            writer.abort();
            return;
        };
        if hello.protocol != croncave_proto::PROTOCOL_VERSION {
            tracing::warn!(%computer, protocol = hello.protocol, "agent protocol mismatch");
            writer.abort();
            return;
        }

        if let Some(old) = self.inner.conns.lock().await.insert(computer, conn.clone()) {
            old.reset_all("the computer reconnected");
        }
        tracing::info!(%computer, version = %hello.agent_version, "agent connected");
        self.inner.hooks.connected(computer, &hello).await;

        let mut heartbeat = {
            let conn = conn.clone();
            let period = self.inner.heartbeat;
            tokio::spawn(async move {
                let mut nonce = 0u64;
                loop {
                    tokio::time::sleep(period).await;
                    if now_secs().saturating_sub(conn.last_pong.load(Ordering::Relaxed)) > period.as_secs() * 3 {
                        tracing::warn!("agent missed heartbeats");
                        break;
                    }
                    nonce += 1;
                    if conn.out.send(Frame::control(&RelayToAgent::Ping { nonce })).await.is_err() {
                        break;
                    }
                }
            })
        };

        loop {
            let msg = tokio::select! {
                m = source.next() => m,
                _ = &mut heartbeat => None,
            };
            let raw = match msg {
                Some(Ok(Message::Binary(raw))) => raw,
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                Some(Ok(_)) => continue,
            };
            let frame = match Frame::decode(raw) {
                Ok(f) => f,
                Err(e) => {
                    tracing::warn!(%computer, error = %e, "bad frame from agent");
                    break;
                }
            };
            self.handle_frame(computer, &conn, frame).await;
        }

        heartbeat.abort();
        writer.abort();
        conn.reset_all("the connection to the computer closed");
        let mut conns = self.inner.conns.lock().await;
        if conns.get(&computer).is_some_and(|c| c.id == conn.id) {
            conns.remove(&computer);
            drop(conns);
            tracing::info!(%computer, "agent disconnected");
            self.inner.hooks.disconnected(computer).await;
        }
    }

    async fn handle_frame(&self, computer: Uuid, conn: &Arc<Conn>, frame: Frame) {
        match frame.kind {
            FrameKind::Control => {
                let msg: AgentToRelay = match serde_json::from_slice(&frame.payload) {
                    Ok(m) => m,
                    Err(e) => {
                        tracing::warn!(%computer, error = %e, "bad control message");
                        return;
                    }
                };
                match msg {
                    AgentToRelay::Hello(_) => {}
                    AgentToRelay::Events { events } => {
                        let upto = events.iter().map(|e| e.seq).max();
                        match self.inner.hooks.events(computer, events).await {
                            Ok(()) => {
                                if let Some(upto) = upto {
                                    let _ = conn.out.send(Frame::control(&RelayToAgent::Ack { upto })).await;
                                }
                            }
                            // Not acknowledged: the agent keeps them and resends.
                            Err(e) => tracing::error!(%computer, error = %e, "storing agent events failed"),
                        }
                    }
                    AgentToRelay::SdkCall { id, call } => {
                        let hooks = self.inner.hooks.clone();
                        let out = conn.out.clone();
                        tokio::spawn(async move {
                            let (ok, value) = match hooks.sdk_call(computer, call).await {
                                Ok(v) => (true, v),
                                Err(e) => (false, Value::String(e)),
                            };
                            let _ = out.send(Frame::control(&RelayToAgent::SdkResult { id, ok, value })).await;
                        });
                    }
                    AgentToRelay::RenewCredential => {
                        let (credential, expires_at) = self.inner.credentials.issue(computer);
                        let _ =
                            conn.out.send(Frame::control(&RelayToAgent::Credential { credential, expires_at })).await;
                    }
                    AgentToRelay::Pong { .. } => conn.last_pong.store(now_secs(), Ordering::Relaxed),
                }
            }
            FrameKind::Data | FrameKind::End | FrameKind::Reset => {
                let tx = conn.streams.lock().expect("stream map").get(&frame.stream).cloned();
                let Some(tx) = tx else { return };
                let item = match frame.kind {
                    FrameKind::Data => StreamItem::Data(frame.payload),
                    FrameKind::End => StreamItem::End,
                    _ => StreamItem::Reset(String::from_utf8_lossy(&frame.payload).into_owned()),
                };
                // Backpressure: wait for the reader instead of dropping bytes.
                let _ = tx.send(item).await;
            }
            FrameKind::Open => tracing::warn!(%computer, "agents may not open streams"),
        }
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}
