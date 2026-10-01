use std::collections::HashMap;
use std::sync::{Arc, Weak};

use croncave_relay::Relay;
use serde::Serialize;
use sqlx::PgPool;
use tokio::sync::{Notify, RwLock, broadcast};
use uuid::Uuid;

use crate::catalog::Catalog;
use crate::clock::Clock;
use crate::config::Config;
use crate::providers::Providers;

pub type AppState = Arc<App>;

/// Everything the control plane shares.
pub struct App {
    pub cfg: Config,
    pub db: PgPool,
    pub clock: Clock,
    pub providers: Providers,
    pub relay: Relay,
    /// Live updates for the web app, fed by Postgres NOTIFY.
    pub live: broadcast::Sender<LiveMsg>,
    /// Wakes the orchestrator and dispatcher early.
    pub kick: Notify,
    pub demo: crate::demo::DemoState,
    /// AI models switched off by an admin (a kill switch per model).
    pub ai_kill: std::sync::RwLock<std::collections::HashSet<String>>,
    catalogs: RwLock<HashMap<i32, Arc<Catalog>>>,
}

/// A small message telling the web app something changed.
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct LiveMsg {
    pub account_id: Uuid,
    pub kind: String,
    pub id: String,
    #[serde(default)]
    pub data: serde_json::Value,
}

impl App {
    pub fn new(cfg: Config, db: PgPool) -> anyhow::Result<AppState> {
        let providers = Providers::from_config(&cfg, db.clone())?;
        let (live, _) = broadcast::channel(1024);
        Ok(Arc::new_cyclic(|weak: &Weak<App>| {
            let hooks = Arc::new(crate::relay_hooks::Hooks { app: weak.clone() });
            let relay = Relay::new(hooks, &cfg.relay_secret, std::time::Duration::from_secs(15 * 60));
            App {
                clock: Clock::new(cfg.adjustable_clock),
                cfg,
                db,
                providers,
                relay,
                live,
                kick: Notify::new(),
                demo: crate::demo::DemoState::default(),
                ai_kill: Default::default(),
                catalogs: RwLock::new(HashMap::new()),
            }
        }))
    }

    pub fn now(&self) -> chrono::DateTime<chrono::Utc> {
        self.clock.now()
    }

    /// Real time, for security expiries (sessions, links, codes, tokens). Business time
    /// (trials, months, schedules) follows [`App::now`], which Dev tools can move.
    pub fn real_now(&self) -> chrono::DateTime<chrono::Utc> {
        chrono::Utc::now()
    }

    pub fn ai_killed(&self, model: &str) -> bool {
        self.ai_kill.read().expect("kill switches").contains(model)
    }

    /// A catalog version (versions never change once saved, so they are cached).
    pub async fn catalog(&self, version: i32) -> anyhow::Result<Arc<Catalog>> {
        if let Some(c) = self.catalogs.read().await.get(&version) {
            return Ok(c.clone());
        }
        let c = Arc::new(crate::catalog::version(&self.db, version).await?);
        self.catalogs.write().await.insert(version, c.clone());
        Ok(c)
    }
}
