//! Configuration from the environment (`.env`). Every provider is chosen here, never by
//! `if dev` branches elsewhere.

use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub api_addr: SocketAddr,
    pub preview_addr: SocketAddr,
    pub preview_domain: String,
    pub web_url: String,
    pub relay_url: String,
    /// An extra address serving only the relay and the demo sites, for computers that
    /// can't reach `api_addr` (Docker containers reach the host through its bridge).
    pub relay_addr: Option<SocketAddr>,
    /// Where computers reach the demo sites the Watcher's demo types read (by default the
    /// relay's address, since computers reach that already).
    pub demo_url: String,
    pub compute_driver: String,
    pub payments: String,
    pub notifier: String,
    pub ai: String,
    pub market_data: String,
    pub data_dir: PathBuf,
    pub agent_bin: PathBuf,
    pub docker_image: String,
    pub fly_api_token: Option<String>,
    pub fly_app: Option<String>,
    pub secrets_key: [u8; 32],
    pub relay_secret: Vec<u8>,
    pub admin_emails: Vec<String>,
    pub dev_tools: bool,
    pub adjustable_clock: bool,
    pub stay_awake_within_secs: i64,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let get = |k: &str, d: &str| std::env::var(k).ok().filter(|v| !v.is_empty()).unwrap_or_else(|| d.to_string());
        let opt = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
        let database_url = opt("DATABASE_URL").ok_or_else(|| anyhow::anyhow!("DATABASE_URL is not set"))?;
        let secrets_key = match opt("SECRETS_KEY") {
            Some(hex_key) => {
                let bytes = hex::decode(hex_key.trim()).map_err(|_| anyhow::anyhow!("SECRETS_KEY must be hex"))?;
                bytes.try_into().map_err(|_| anyhow::anyhow!("SECRETS_KEY must be 32 bytes (64 hex characters)"))?
            }
            None => anyhow::bail!("SECRETS_KEY is not set (scripts/dev.sh generates one in .env)"),
        };
        let relay_secret = opt("RELAY_SECRET")
            .ok_or_else(|| anyhow::anyhow!("RELAY_SECRET is not set (scripts/dev.sh generates one in .env)"))?
            .into_bytes();
        let relay_url = get("RELAY_URL", "http://127.0.0.1:8080");
        Ok(Self {
            database_url,
            api_addr: get("API_ADDR", "127.0.0.1:8080").parse()?,
            preview_addr: get("PREVIEW_ADDR", "127.0.0.1:8081").parse()?,
            preview_domain: get("PREVIEW_DOMAIN", "preview.localhost:8081"),
            web_url: get("WEB_URL", "http://localhost:5173"),
            demo_url: opt("DEMO_URL").unwrap_or_else(|| relay_url.clone()),
            relay_url,
            relay_addr: opt("RELAY_ADDR").map(|a| a.parse()).transpose()?,
            compute_driver: get("COMPUTE_DRIVER", "local"),
            payments: get("PAYMENTS", "mock"),
            notifier: get("NOTIFIER", "outbox"),
            ai: get("AI", "mock"),
            market_data: get("MARKET_DATA", "mock"),
            data_dir: PathBuf::from(get("DATA_DIR", ".dev/data")),
            agent_bin: PathBuf::from(get("AGENT_BIN", "target/debug/croncave-agent")),
            docker_image: get("COMPUTER_IMAGE", "croncave-computer:dev"),
            fly_api_token: opt("FLY_API_TOKEN"),
            fly_app: opt("FLY_APP"),
            secrets_key,
            relay_secret,
            admin_emails: get("ADMIN_EMAILS", "")
                .split(',')
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty())
                .collect(),
            dev_tools: get("DEV_TOOLS", "false") == "true",
            adjustable_clock: get("CLOCK", "system") == "adjustable",
            stay_awake_within_secs: get("STAY_AWAKE_WITHIN_SECS", "300").parse()?,
        })
    }

    /// A configuration for tests.
    pub fn for_tests(database_url: &str, data_dir: PathBuf) -> Self {
        Self {
            database_url: database_url.to_string(),
            api_addr: "127.0.0.1:0".parse().expect("valid"),
            preview_addr: "127.0.0.1:0".parse().expect("valid"),
            preview_domain: "preview.localhost".into(),
            web_url: "http://localhost:5173".into(),
            relay_url: "http://127.0.0.1:0".into(),
            relay_addr: None,
            demo_url: "http://127.0.0.1:8080".into(),
            compute_driver: "local".into(),
            payments: "mock".into(),
            notifier: "outbox".into(),
            ai: "mock".into(),
            market_data: "mock".into(),
            data_dir,
            agent_bin: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/croncave-agent"),
            docker_image: "croncave-computer:dev".into(),
            fly_api_token: None,
            fly_app: None,
            secrets_key: [7u8; 32],
            relay_secret: b"test-relay-secret".to_vec(),
            admin_emails: vec!["admin@croncave.local".into()],
            dev_tools: true,
            adjustable_clock: true,
            stay_awake_within_secs: 300,
        }
    }
}
