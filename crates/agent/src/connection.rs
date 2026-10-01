//! The one outgoing connection: credential exchange, reconnect with backoff, and the
//! frame loop that feeds streams and control messages.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use croncave_proto::{AgentToRelay, Frame, FrameKind, Hello, PROTOCOL_VERSION, RelayToAgent, StreamOpen};
use futures::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

use crate::Agent;
use crate::stream::{AgentStream, Incoming};

enum Ended {
    /// The relay asked us to sleep: exit cleanly.
    Sleep,
    /// The connection dropped: reconnect.
    Dropped,
    /// The credential was refused and the bootstrap is spent: nothing more we can do.
    Unauthorized,
}

pub async fn run(agent: Arc<Agent>) -> anyhow::Result<()> {
    let http = reqwest::Client::builder().no_proxy().timeout(Duration::from_secs(10)).build()?;
    let mut credential: Option<String> = None;
    let mut bootstrap = Some(agent.config.bootstrap_token.clone());
    let mut backoff = Duration::from_millis(250);

    loop {
        if credential.is_none() {
            let Some(token) = bootstrap.take() else {
                anyhow::bail!("the credential expired and the bootstrap token was already used");
            };
            match exchange(&http, &agent.config.relay_url, &token).await {
                Ok(c) => credential = Some(c),
                Err(e) => {
                    tracing::warn!(error = %e, "could not trade the bootstrap token; retrying");
                    bootstrap = Some(token);
                    tokio::time::sleep(backoff).await;
                    backoff = (backoff * 2).min(Duration::from_secs(10));
                    continue;
                }
            }
        }
        let cred = credential.clone().expect("set above");
        match session(&agent, &cred, &mut credential).await {
            Ok(Ended::Sleep) => return Ok(()),
            Ok(Ended::Unauthorized) => {
                credential = None;
                if bootstrap.is_none() {
                    anyhow::bail!("the relay refused this computer's credential");
                }
            }
            Ok(Ended::Dropped) => backoff = Duration::from_millis(250),
            Err(e) => tracing::warn!(error = %e, "connection to the relay failed"),
        }
        *agent.link.lock().await = None;
        tokio::time::sleep(backoff).await;
        backoff = (backoff * 2).min(Duration::from_secs(10));
    }
}

async fn exchange(http: &reqwest::Client, relay: &str, token: &str) -> anyhow::Result<String> {
    #[derive(serde::Deserialize)]
    struct Resp {
        credential: String,
    }
    let resp = http
        .post(format!("{relay}/relay/token"))
        .json(&serde_json::json!({ "bootstrap": token }))
        .send()
        .await?
        .error_for_status()?;
    Ok(resp.json::<Resp>().await?.credential)
}

async fn session(agent: &Arc<Agent>, cred: &str, credential: &mut Option<String>) -> anyhow::Result<Ended> {
    let ws_url = agent.config.relay_url.replacen("http", "ws", 1) + "/relay/connect";
    let mut req = ws_url.into_client_request()?;
    req.headers_mut().insert("authorization", format!("Bearer {cred}").parse()?);
    let (ws, _) = match tokio_tungstenite::connect_async(req).await {
        Ok(ok) => ok,
        Err(tokio_tungstenite::tungstenite::Error::Http(resp)) if resp.status() == 401 => {
            return Ok(Ended::Unauthorized);
        }
        Err(e) => return Err(e.into()),
    };
    let (mut sink, mut source) = ws.split();
    let (out_tx, mut out_rx) = mpsc::channel::<Frame>(256);

    let hello = Hello {
        computer_id: agent.config.computer_id,
        agent_version: crate::VERSION.to_string(),
        protocol: PROTOCOL_VERSION,
        outbox_epoch: agent.outbox.epoch(),
        running_runs: agent.running_runs(),
    };
    sink.send(Message::Binary(Frame::control(&AgentToRelay::Hello(hello)).encode())).await?;
    *agent.link.lock().await = Some(out_tx.clone());
    agent.outbox.rewind();
    tracing::info!("connected to the relay");

    let writer = tokio::spawn(async move {
        while let Some(f) = out_rx.recv().await {
            if sink.send(Message::Binary(f.encode())).await.is_err() {
                break;
            }
        }
        let _ = sink.close().await;
    });

    // Send events as they arrive, in batches.
    let sender = {
        let agent = agent.clone();
        let out = out_tx.clone();
        tokio::spawn(async move {
            loop {
                let batch = agent.outbox.take_unsent(500);
                if batch.is_empty() {
                    tokio::select! {
                        _ = agent.outbox.wait() => {}
                        _ = tokio::time::sleep(Duration::from_millis(500)) => {}
                    }
                    continue;
                }
                if out.send(Frame::control(&AgentToRelay::Events { events: batch })).await.is_err() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
    };

    // Renew the credential well before it expires.
    let renew = {
        let out = out_tx.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(120)).await;
                if out.send(Frame::control(&AgentToRelay::RenewCredential)).await.is_err() {
                    break;
                }
            }
        })
    };

    let mut streams: HashMap<u32, mpsc::Sender<Incoming>> = HashMap::new();
    let ended = loop {
        let raw = match source.next().await {
            Some(Ok(Message::Binary(raw))) => raw,
            Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break Ended::Dropped,
            Some(Ok(_)) => continue,
        };
        let Ok(frame) = Frame::decode(Bytes::from(raw.to_vec())) else { break Ended::Dropped };
        match frame.kind {
            FrameKind::Control => {
                let Ok(msg) = serde_json::from_slice::<RelayToAgent>(&frame.payload) else { continue };
                match msg {
                    RelayToAgent::Ping { nonce } => {
                        let _ = out_tx.send(Frame::control(&AgentToRelay::Pong { nonce })).await;
                    }
                    RelayToAgent::Credential { credential: c, .. } => *credential = Some(c),
                    RelayToAgent::Sleep => break Ended::Sleep,
                    other => {
                        let agent = agent.clone();
                        tokio::spawn(async move { agent.handle_control(other).await });
                    }
                }
            }
            FrameKind::Open => {
                let open: StreamOpen = match serde_json::from_slice(&frame.payload) {
                    Ok(o) => o,
                    Err(_) => {
                        let _ = out_tx.send(Frame::new(FrameKind::Reset, frame.stream, "unknown stream")).await;
                        continue;
                    }
                };
                let (tx, rx) = mpsc::channel(64);
                streams.insert(frame.stream, tx);
                let stream = AgentStream::new(frame.stream, rx, out_tx.clone());
                tokio::spawn(handle_stream(agent.clone(), open, stream));
            }
            FrameKind::Data => {
                if let Some(tx) = streams.get(&frame.stream) {
                    let _ = tx.send(Incoming::Data(frame.payload)).await;
                }
            }
            FrameKind::End => {
                if let Some(tx) = streams.remove(&frame.stream) {
                    let _ = tx.send(Incoming::End).await;
                }
            }
            FrameKind::Reset => {
                if let Some(tx) = streams.remove(&frame.stream) {
                    let _ = tx.send(Incoming::Reset).await;
                }
            }
        }
        streams.retain(|_, tx| !tx.is_closed());
    };

    sender.abort();
    renew.abort();
    *agent.link.lock().await = None;
    drop(out_tx);
    let _ = tokio::time::timeout(Duration::from_secs(2), writer).await;
    Ok(ended)
}

async fn handle_stream(agent: Arc<Agent>, open: StreamOpen, mut stream: AgentStream) {
    match open {
        StreamOpen::Files(req) => crate::files::handle(&agent, req, &mut stream).await,
        StreamOpen::PreviewHttp(head) => crate::preview::handle(head, &mut stream).await,
        StreamOpen::CodeReview(req) => crate::coder::review(&agent, req, &mut stream).await,
    }
    stream.end().await;
}
