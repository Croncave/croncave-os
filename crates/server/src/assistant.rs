//! The AI gateway and the assistant. Off until the person turns it on. Every call is
//! routed to a model, checked against the spending cap first, metered in tokens and
//! billed at exactly the provider's rate. The assistant only proposes; applying a
//! proposal is a normal API call the person makes.

use axum::Json;
use axum::extract::State;
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::auth::Auth;
use crate::error::{ApiError, ApiResult};
use crate::providers::ai::AiRequest;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct Ask {
    pub message: String,
    #[serde(default)]
    pub context: Value,
}

const SYSTEM: &str = "You are Croncave's assistant. You help people set up watches and scripts on their private cloud computer. \
You never act directly: reply briefly and return proposals as setups the person can apply.";

pub async fn ask(State(app): State<AppState>, auth: Auth, Json(b): Json<Ask>) -> ApiResult<Json<Value>> {
    if !auth.account.ai_enabled {
        return Err(ApiError::conflict("The assistant is off. Turn it on in Settings to use it.").with_code("ai_off"));
    }
    let message = b.message.trim();
    if message.is_empty() {
        return Err(ApiError::bad("Ask something."));
    }
    let model = app.providers.ai.clone();
    if app.ai_killed(model.id()) {
        return Err(ApiError::unavailable(
            "The assistant is switched off for maintenance. Everything else works as usual.",
        ));
    }
    // The cap is checked before every call.
    if crate::billing::is_paused(&app.db, auth.account.id).await? {
        return Err(
            ApiError::limit("The assistant stops at your spending cap. Raise the cap to use it.").with_code("paused")
        );
    }
    let history: Vec<(String, String)> = sqlx::query_as(
        "select role, text from (select role, text, created_at from assistant_messages where account_id = $1 order by created_at desc limit 10) m order by created_at",
    )
    .bind(auth.account.id)
    .fetch_all(&app.db)
    .await?;
    let mut messages = history;
    messages.push(("user".into(), message.to_string()));
    let req = AiRequest { feature: "assistant".into(), system: SYSTEM.into(), messages, context: b.context.clone() };
    let resp =
        model.complete(&req).await.map_err(|e| ApiError::unavailable(format!("The assistant couldn't answer: {e}")))?;
    let parsed: Value =
        serde_json::from_str(&resp.text).unwrap_or_else(|_| json!({ "reply": resp.text, "proposals": [] }));
    let catalog = app.catalog(auth.account.catalog_version).await?;
    let cost = catalog.ai_cost_micros(&resp.model, resp.tokens_in, resp.tokens_out);
    let now = app.now();
    sqlx::query(
        "insert into assistant_messages (id, account_id, role, text, created_at) values ($1, $2, 'user', $3, $4)",
    )
    .bind(Uuid::new_v4())
    .bind(auth.account.id)
    .bind(message)
    .bind(now)
    .execute(&app.db)
    .await?;
    sqlx::query(
        "insert into assistant_messages (id, account_id, role, text, proposals, tokens_in, tokens_out, cost_micros, created_at)
         values ($1, $2, 'assistant', $3, $4, $5, $6, $7, $8)",
    )
    .bind(Uuid::new_v4())
    .bind(auth.account.id)
    .bind(parsed["reply"].as_str().unwrap_or_default())
    .bind(&parsed["proposals"])
    .bind(resp.tokens_in as i32)
    .bind(resp.tokens_out as i32)
    .bind(cost)
    .bind(now + chrono::Duration::milliseconds(1))
    .execute(&app.db)
    .await?;
    crate::billing::record_usage(
        &app,
        auth.account.id,
        cost,
        None,
        "ai",
        json!({ "model": resp.model, "tokens_in": resp.tokens_in, "tokens_out": resp.tokens_out }),
    )
    .await?;
    crate::measure::audit(
        &app.db,
        auth.account.id,
        "assistant",
        None,
        "assistant.answered",
        "",
        json!({ "proposals": parsed["proposals"].as_array().map(|a| a.len()) }),
    )
    .await?;
    Ok(Json(json!({
        "reply": parsed["reply"], "proposals": parsed["proposals"], "model": resp.model,
        "tokens_in": resp.tokens_in, "tokens_out": resp.tokens_out, "cost_micros": cost,
    })))
}

pub async fn history(State(app): State<AppState>, auth: Auth) -> ApiResult<Json<Value>> {
    let rows: Vec<(String, String, Value, i64, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        "select role, text, proposals, cost_micros, created_at from (select * from assistant_messages where account_id = $1 order by created_at desc limit 40) m order by created_at",
    )
    .bind(auth.account.id)
    .fetch_all(&app.db)
    .await?;
    Ok(Json(
        json!({ "messages": rows.into_iter().map(|(r, t, p, c, at)| json!({ "role": r, "text": t, "proposals": p, "cost_micros": c, "at": at })).collect::<Vec<_>>(), "enabled": auth.account.ai_enabled }),
    ))
}
