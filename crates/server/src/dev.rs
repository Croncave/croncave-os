//! Dev tools, mounted only when `DEV_TOOLS=true`: the notifier outbox (sign-in links and
//! codes), moving the clock forward, simulated usage and the demo sites' controls.

use axum::Json;
use axum::extract::State;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::auth::Auth;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

pub async fn outbox(State(app): State<AppState>) -> ApiResult<Json<Value>> {
    let rows: Vec<(uuid::Uuid, String, String, String, String, Option<String>, chrono::DateTime<chrono::Utc>)> =
        sqlx::query_as("select id, channel, recipient, subject, body, link, created_at from outbox order by created_at desc limit 100").fetch_all(&app.db).await?;
    Ok(Json(
        json!({ "messages": rows.into_iter().map(|(id, ch, to, s, b, l, at)| json!({ "id": id, "channel": ch, "to": to, "subject": s, "body": b, "link": l, "at": at })).collect::<Vec<_>>() }),
    ))
}

pub async fn state(State(app): State<AppState>) -> Json<Value> {
    Json(json!({
        "clock_offset_secs": app.clock.offset_secs(),
        "now": app.now(),
        "listings": app.demo.listings.lock().expect("listings").len(),
        "page": app.demo.page.lock().expect("page").1,
        "drivers": { "compute": app.providers.compute.name(), "payments": app.cfg.payments, "notifier": app.cfg.notifier, "ai": app.cfg.ai, "market_data": app.cfg.market_data },
    }))
}

#[derive(Deserialize)]
pub struct Advance {
    pub secs: i64,
}

/// Move time forward to end trials and months; jobs that came due run once.
pub async fn advance_clock(State(app): State<AppState>, Json(b): Json<Advance>) -> ApiResult<Json<Value>> {
    app.clock.advance(b.secs).map_err(ApiError::bad)?;
    app.kick.notify_waiters();
    Ok(Json(json!({ "offset_secs": app.clock.offset_secs(), "now": app.now() })))
}

#[derive(Deserialize)]
pub struct Usage {
    pub dollars: f64,
}

/// Add usage to your own account, through the same ledger path as real metering.
pub async fn simulate_usage(State(app): State<AppState>, auth: Auth, Json(b): Json<Usage>) -> ApiResult<Json<Value>> {
    if !(0.0..=1000.0).contains(&b.dollars) {
        return Err(ApiError::bad("Between $0 and $1,000."));
    }
    crate::billing::record_usage(
        &app,
        auth.account.id,
        crate::catalog::micros(b.dollars),
        None,
        "simulated",
        json!({ "note": "Dev tools" }),
    )
    .await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn add_listing(State(app): State<AppState>) -> Json<Value> {
    let l = app.demo.add_listing();
    Json(json!({ "id": l.id, "title": l.title, "price": l.price, "beds": l.beds, "area": l.area }))
}

pub async fn change_page(State(app): State<AppState>) -> Json<Value> {
    Json(json!({ "text": app.demo.change_page() }))
}

#[derive(Deserialize)]
pub struct Nudge {
    pub symbol: String,
    pub percent: f64,
}

pub async fn nudge_stock(State(app): State<AppState>, Json(b): Json<Nudge>) -> ApiResult<Json<Value>> {
    let m = app
        .providers
        .mock_market
        .as_ref()
        .ok_or_else(|| ApiError::bad("Prices can only be nudged with MARKET_DATA=mock."))?;
    m.nudge(&b.symbol, b.percent);
    let q = app.providers.market.quote(&b.symbol, app.now()).await.map_err(ApiError::bad)?;
    Ok(Json(json!({ "symbol": q.symbol, "price": q.price })))
}
