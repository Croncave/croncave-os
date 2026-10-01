//! The Croncave agent: the one program on a computer that talks to the platform.
//!
//! It dials out to the relay and never listens. Everything (runs, files, previews) travels
//! over that one connection. Runs are supervised by the agent, so they keep going while
//! the connection drops; events are kept on disk until the relay acknowledges them.

pub mod coder;
pub mod connection;
pub mod disk;
pub mod files;
pub mod outbox;
pub mod preview;
pub mod runs;
pub mod scripts;
pub mod snapshot;
pub mod stream;
pub mod watcher;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use croncave_proto::{AgentEvent, Frame, RelayToAgent, SdkCall};
use serde_json::Value;
use tokio::sync::{Mutex, mpsc, oneshot};
use uuid::Uuid;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone)]
pub struct AgentConfig {
    /// Base URL of the relay, e.g. `http://127.0.0.1:8080`.
    pub relay_url: String,
    pub computer_id: Uuid,
    /// One-time token the orchestrator gave this boot.
    pub bootstrap_token: String,
    /// The computer's disk.
    pub disk: PathBuf,
    /// How long Trash keeps items.
    pub trash_days: u64,
}

impl AgentConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let var = |k: &str| std::env::var(k).map_err(|_| anyhow::anyhow!("{k} is not set"));
        Ok(Self {
            relay_url: var("CRONCAVE_RELAY_URL")?,
            computer_id: var("CRONCAVE_COMPUTER_ID")?.parse()?,
            bootstrap_token: var("CRONCAVE_BOOTSTRAP_TOKEN")?,
            disk: PathBuf::from(var("CRONCAVE_DISK")?),
            trash_days: std::env::var("CRONCAVE_TRASH_DAYS").ok().and_then(|v| v.parse().ok()).unwrap_or(30),
        })
    }
}

/// Shared state of a running agent.
pub struct Agent {
    pub config: AgentConfig,
    pub disk: disk::Disk,
    pub outbox: outbox::Outbox,
    /// Sender for the current connection, if connected.
    link: Mutex<Option<mpsc::Sender<Frame>>>,
    pub runs: runs::Supervisor,
    sdk_calls: Mutex<HashMap<u64, oneshot::Sender<Result<Value, String>>>>,
    next_sdk_call: std::sync::atomic::AtomicU64,
}

impl Agent {
    pub async fn new(config: AgentConfig) -> anyhow::Result<Arc<Self>> {
        let disk = disk::Disk::open(&config.disk)?;
        let outbox = outbox::Outbox::open(disk.system("agent"))?;
        Ok(Arc::new(Self {
            config,
            disk,
            outbox,
            link: Mutex::new(None),
            runs: runs::Supervisor::default(),
            sdk_calls: Mutex::new(HashMap::new()),
            next_sdk_call: std::sync::atomic::AtomicU64::new(1),
        }))
    }

    /// Record an event durably; it is sent when connected.
    pub fn emit(&self, event: AgentEvent) {
        if let Err(e) = self.outbox.push(event) {
            tracing::error!(error = %e, "could not record event");
        }
    }

    pub(crate) async fn send(&self, frame: Frame) -> bool {
        let link = self.link.lock().await.clone();
        match link {
            Some(tx) => tx.send(frame).await.is_ok(),
            None => false,
        }
    }

    /// Ask the platform for something (keys stay in the control plane).
    pub async fn sdk(&self, call: SdkCall) -> Result<Value, String> {
        let id = self.next_sdk_call.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.sdk_calls.lock().await.insert(id, tx);
        let msg = croncave_proto::AgentToRelay::SdkCall { id, call };
        if !self.send(Frame::control(&msg)).await {
            self.sdk_calls.lock().await.remove(&id);
            return Err("the computer isn't connected to Croncave right now".into());
        }
        match tokio::time::timeout(std::time::Duration::from_secs(20), rx).await {
            Ok(Ok(r)) => r,
            _ => {
                self.sdk_calls.lock().await.remove(&id);
                Err("the platform didn't answer in time".into())
            }
        }
    }

    pub(crate) async fn handle_control(self: &Arc<Self>, msg: RelayToAgent) {
        match msg {
            RelayToAgent::Ack { upto } => self.outbox.ack(upto),
            RelayToAgent::StartRun(spec) => runs::start(self.clone(), spec).await,
            RelayToAgent::StopRun { run_id } => self.runs.stop(run_id).await,
            RelayToAgent::Approval { run_id, request_id, approved } => {
                self.runs.answer(run_id, &request_id, approved).await
            }
            RelayToAgent::SdkResult { id, ok, value } => {
                if let Some(tx) = self.sdk_calls.lock().await.remove(&id) {
                    let _ = tx.send(if ok { Ok(value) } else { Err(value.as_str().unwrap_or("failed").to_string()) });
                }
            }
            // Handled by the connection loop.
            RelayToAgent::Credential { .. } | RelayToAgent::Ping { .. } | RelayToAgent::Sleep => {}
        }
    }

    pub fn running_runs(&self) -> Vec<Uuid> {
        self.runs.running()
    }
}

/// Run the agent until it is told to sleep.
pub async fn run(config: AgentConfig) -> anyhow::Result<()> {
    let agent = Agent::new(config).await?;
    // Anything still running from a previous boot ends now, as it would when a machine restarts.
    runs::kill_leftovers(&agent.disk).await;
    files::clean_trash(&agent.disk, agent.config.trash_days);
    let health = tokio::spawn(runs::report_health(agent.clone()));
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    let result = tokio::select! {
        r = connection::run(agent.clone()) => r,
        _ = terminate.recv() => {
            tracing::info!("asked to stop; ending runs");
            Ok(())
        }
    };
    health.abort();
    agent.runs.stop_all().await;
    agent.outbox.flush_to_disk();
    result
}
