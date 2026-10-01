//! Measurements from day one: plan funnel events, wake times and why computers were
//! awake. Counts, durations and costs only; never file contents, output or prompts.

use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

/// Record a plan funnel or activation event. Measurement never breaks the product, so
/// failures are logged and swallowed.
pub async fn funnel(db: &PgPool, account: Option<Uuid>, kind: &str, data: Value) {
    if let Err(e) = sqlx::query("insert into funnel_events (account_id, kind, data) values ($1, $2, $3)")
        .bind(account)
        .bind(kind)
        .bind(data)
        .execute(db)
        .await
    {
        tracing::warn!(error = %e, kind, "could not record a funnel event");
    }
}

/// Record who did what, for the audit log people see in Settings.
pub async fn audit<'e, E: sqlx::PgExecutor<'e>>(
    ex: E,
    account: Uuid,
    actor_kind: &str,
    actor_id: Option<Uuid>,
    action: &str,
    target: &str,
    data: Value,
) -> sqlx::Result<()> {
    sqlx::query("insert into audit_log (account_id, actor_kind, actor_id, action, target, data) values ($1, $2, $3, $4, $5, $6)")
        .bind(account)
        .bind(actor_kind)
        .bind(actor_id)
        .bind(action)
        .bind(target)
        .bind(data)
        .execute(ex)
        .await?;
    Ok(())
}
