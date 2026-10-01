//! Email, text messages and push go through one interface. The `outbox` implementation
//! stores every message, and the Dev tools page shows it.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    Email,
    Sms,
    Push,
}

impl Channel {
    pub fn as_str(self) -> &'static str {
        match self {
            Channel::Email => "email",
            Channel::Sms => "sms",
            Channel::Push => "push",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Message {
    pub channel: Channel,
    pub to: String,
    pub subject: String,
    pub body: String,
    pub link: Option<String>,
}

#[async_trait]
pub trait Notifier: Send + Sync {
    /// Send a message; returns the provider's id for it.
    async fn send(&self, msg: Message) -> anyhow::Result<Uuid>;
}

pub struct OutboxNotifier {
    db: PgPool,
}

impl OutboxNotifier {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }
}

#[async_trait]
impl Notifier for OutboxNotifier {
    async fn send(&self, msg: Message) -> anyhow::Result<Uuid> {
        let id = Uuid::new_v4();
        sqlx::query("insert into outbox (id, channel, recipient, subject, body, link) values ($1, $2, $3, $4, $5, $6)")
            .bind(id)
            .bind(msg.channel.as_str())
            .bind(&msg.to)
            .bind(&msg.subject)
            .bind(&msg.body)
            .bind(&msg.link)
            .execute(&self.db)
            .await?;
        tracing::info!(channel = msg.channel.as_str(), "message stored in the outbox");
        Ok(id)
    }
}
