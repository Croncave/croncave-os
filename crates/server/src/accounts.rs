//! The signed-in person: who they are, where they are in sign-up, their preferences.

use axum::Json;
use axum::extract::{Path, State};
use chrono::Duration;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::auth::Auth;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

/// `GET /api/me`: everything the shell needs (top bar, sign-up step, computers).
pub async fn me(State(app): State<AppState>, auth: Auth) -> ApiResult<Json<Value>> {
    let now = app.now();
    // "Since you left": remember the previous visit when someone comes back after a while.
    let gap = auth.user.last_seen_at.is_none_or(|t| now - t > Duration::minutes(30));
    let (previous,): (Option<chrono::DateTime<chrono::Utc>>,) = sqlx::query_as(
        "update users set previous_seen_at = case when $2 then last_seen_at else previous_seen_at end, last_seen_at = $3
         where id = $1 returning previous_seen_at",
    )
    .bind(auth.user.id)
    .bind(gap)
    .bind(now)
    .fetch_one(&app.db)
    .await?;

    let step = if auth.user.phone_verified_at.is_none() {
        "phone"
    } else if auth.account.plan.is_none() {
        "plan"
    } else if auth.account.signup_completed_at.is_none() {
        "trial"
    } else {
        "done"
    };
    let computers = crate::computers::list_for(&app, auth.account.id).await?;
    let unread = crate::events::unread_count(&app, auth.account.id).await?;
    let mut usage = Value::Null;
    if auth.account.plan.is_some() {
        let (_, plan) = crate::billing::plan_of(&app, &auth.account).await?;
        let mut conn = app.db.acquire().await?;
        let b = crate::billing::balances(&mut conn, auth.account.id, auth.account.period_start).await?;
        let (spent,): (i64,) = sqlx::query_as(
            "select coalesce(-sum(amount_micros), 0)::bigint from ledger_entries where account_id = $1 and kind = 'usage' and period_start = $2 and bucket <> 'unbilled'",
        )
        .bind(auth.account.id)
        .bind(auth.account.period_start)
        .fetch_one(&mut *conn)
        .await?;
        usage = json!({
            "left_micros": b.award + b.credit + b.trial + b.allowance.max(0),
            "spent_this_month_micros": spent,
            "cap_micros": crate::billing::cap_rules(&auth.account, &plan).cap,
            "plan_name": plan.name,
        });
    }
    Ok(Json(json!({
        "user": { "id": auth.user.id, "email": auth.user.email, "name": auth.user.name, "phone": auth.user.phone,
                  "is_admin": auth.user.is_admin, "prefs": auth.user.prefs, "previous_seen_at": previous },
        "account": auth.account,
        "signup_step": step,
        "computers": computers,
        "unread": unread,
        "usage": usage,
        "dev_tools": app.cfg.dev_tools,
        "clock_offset_secs": app.clock.offset_secs(),
        "now": now,
    })))
}

#[derive(Deserialize)]
pub struct PrefsBody {
    pub name: Option<String>,
    pub mode: Option<String>,
    pub channels: Option<Value>,
    pub quiet_hours: Option<Value>,
    pub ai_enabled: Option<bool>,
    /// Hide the floating Ask button (it can be shown again from Settings › Assistant).
    pub ask_hidden: Option<bool>,
}

pub async fn update_prefs(State(app): State<AppState>, auth: Auth, Json(b): Json<PrefsBody>) -> ApiResult<Json<Value>> {
    let mut prefs = auth.user.prefs.clone();
    if let Some(m) = &b.mode {
        if !["dark", "light", "system"].contains(&m.as_str()) {
            return Err(ApiError::bad("Choose dark, light or match the system."));
        }
        prefs["mode"] = json!(m);
    }
    if let Some(c) = b.channels {
        prefs["channels"] = json!({
            "email": c["email"].as_bool().unwrap_or(true),
            "sms": c["sms"].as_bool().unwrap_or(false),
            "push": c["push"].as_bool().unwrap_or(false),
        });
    }
    if let Some(q) = b.quiet_hours {
        prefs["quiet_hours"] = q;
    }
    if let Some(h) = b.ask_hidden {
        prefs["ask_hidden"] = json!(h);
    }
    let name = b.name.map(|n| n.trim().chars().take(60).collect::<String>()).unwrap_or(auth.user.name.clone());
    sqlx::query("update users set prefs = $2, name = $3 where id = $1")
        .bind(auth.user.id)
        .bind(&prefs)
        .bind(&name)
        .execute(&app.db)
        .await?;
    if let Some(ai) = b.ai_enabled {
        sqlx::query("update accounts set ai_enabled = $2 where id = $1")
            .bind(auth.account.id)
            .bind(ai)
            .execute(&app.db)
            .await?;
        crate::measure::audit(
            &app.db,
            auth.account.id,
            auth.actor_kind,
            Some(auth.user.id),
            if ai { "assistant.on" } else { "assistant.off" },
            "",
            json!({}),
        )
        .await?;
    }
    Ok(Json(json!({ "ok": true, "prefs": prefs })))
}

/// `GET /api/schemes/:id`: a color scheme is data (light and dark values per token).
pub async fn scheme(State(app): State<AppState>, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    let row: Option<(Value,)> =
        sqlx::query_as("select data from color_schemes where id = $1 order by version desc limit 1")
            .bind(&id)
            .fetch_optional(&app.db)
            .await?;
    row.map(|(d,)| Json(d)).ok_or_else(|| ApiError::not_found("That color scheme"))
}

pub async fn audit_log(State(app): State<AppState>, auth: Auth) -> ApiResult<Json<Value>> {
    let rows: Vec<(String, String, String, Value, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        "select actor_kind, action, target, data, at from audit_log where account_id = $1 order by id desc limit 100",
    )
    .bind(auth.account.id)
    .fetch_all(&app.db)
    .await?;
    Ok(Json(
        json!({ "entries": rows.into_iter().map(|(a, act, t, d, at)| json!({ "actor": a, "action": act, "target": t, "data": d, "at": at })).collect::<Vec<_>>() }),
    ))
}
