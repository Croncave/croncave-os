//! Admin: the plan catalog editor, account credits, and the measurements dashboard that
//! compares what the alpha measures with the pricing model's assumptions.

use axum::Json;
use axum::extract::{Path, State};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::auth::Admin;
use crate::catalog::{Catalog, CatalogVersion, micros};
use crate::error::{ApiError, ApiResult};
use crate::events::{self, NewEvent};
use crate::ledger::Bucket;
use crate::state::AppState;

pub async fn catalog(State(app): State<AppState>, Admin(_): Admin) -> ApiResult<Json<Value>> {
    let versions: Vec<CatalogVersion> =
        sqlx::query_as("select id, starts_at, note, created_at from catalog_versions order by id desc")
            .fetch_all(&app.db)
            .await?;
    let (current, data) = crate::catalog::current(&app.db, app.now()).await?;
    let counts: Vec<(i32, i64)> =
        sqlx::query_as("select catalog_version, count(*) from accounts group by 1").fetch_all(&app.db).await?;
    Ok(Json(json!({
        "current": current, "data": data,
        "versions": versions.into_iter().map(|v| { let n = counts.iter().find(|c| c.0 == v.id).map(|c| c.1).unwrap_or(0); json!({ "id": v.id, "starts_at": v.starts_at, "note": v.note, "created_at": v.created_at, "accounts": n }) }).collect::<Vec<_>>(),
    })))
}

#[derive(Deserialize)]
pub struct NewVersion {
    pub data: Value,
    pub note: String,
    pub starts_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Save a new catalog version. Existing accounts stay on the version they signed up on.
pub async fn save_catalog(
    State(app): State<AppState>,
    Admin(auth): Admin,
    Json(b): Json<NewVersion>,
) -> ApiResult<Json<Value>> {
    let c: Catalog =
        serde_json::from_value(b.data).map_err(|e| ApiError::bad(format!("The catalog isn't complete: {e}")))?;
    let problems = c.problems();
    if !problems.is_empty() {
        return Err(ApiError::bad(problems.join(" ")));
    }
    if b.note.trim().is_empty() {
        return Err(ApiError::bad("Say what changed in a short note."));
    }
    let (id,): (i32,) = sqlx::query_as(
        "insert into catalog_versions (starts_at, data, note, created_by) values ($1, $2, $3, $4) returning id",
    )
    .bind(b.starts_at.unwrap_or(app.now()))
    .bind(serde_json::to_value(&c).expect("serializes"))
    .bind(b.note.trim())
    .bind(auth.user.id)
    .fetch_one(&app.db)
    .await?;
    crate::measure::audit(
        &app.db,
        auth.account.id,
        "user",
        Some(auth.user.id),
        "catalog.version_created",
        &id.to_string(),
        json!({ "note": b.note }),
    )
    .await?;
    Ok(Json(json!({ "id": id })))
}

pub async fn accounts(State(app): State<AppState>, Admin(_): Admin) -> ApiResult<Json<Value>> {
    let rows: Vec<(Uuid, String, Option<String>, Option<String>, i32, Option<chrono::DateTime<chrono::Utc>>, chrono::DateTime<chrono::Utc>, i64, i64, i64)> = sqlx::query_as(
        "select a.id, u.email, a.plan, a.trial_plan, a.catalog_version, a.paused_at, a.created_at,
                (select count(*) from computers c where c.account_id = a.id and c.deleted_at is null),
                coalesce((select sum(amount_micros) from ledger_entries l where l.account_id = a.id and l.bucket in ('award', 'credit', 'trial', 'allowance')), 0)::bigint,
                coalesce((select -sum(amount_micros) from ledger_entries l where l.account_id = a.id and l.kind = 'usage' and l.period_start = a.period_start), 0)::bigint
         from accounts a join account_members m on m.account_id = a.id join users u on u.id = m.user_id
         order by a.created_at desc limit 500",
    )
    .fetch_all(&app.db)
    .await?;
    Ok(Json(
        json!({ "accounts": rows.into_iter().map(|(id, email, plan, trial, v, paused, created, computers, left, used)| json!({
        "id": id, "email": email, "plan": plan, "trial_plan": trial, "catalog_version": v, "paused": paused.is_some(),
        "created_at": created, "computers": computers, "left_micros": left, "used_this_period_micros": used,
    })).collect::<Vec<_>>() }),
    ))
}

#[derive(Deserialize)]
pub struct Credit {
    pub dollars: f64,
    pub reason: String,
    pub ends_at: Option<chrono::DateTime<chrono::Utc>>,
}

pub async fn credit(
    State(app): State<AppState>,
    Admin(auth): Admin,
    Path(id): Path<Uuid>,
    Json(b): Json<Credit>,
) -> ApiResult<Json<Value>> {
    if !(0.01..=1000.0).contains(&b.dollars) {
        return Err(ApiError::bad("Give between $0.01 and $1,000."));
    }
    if b.reason.trim().is_empty() {
        return Err(ApiError::bad("Every credit needs a reason."));
    }
    let mut tx = app.db.begin().await?;
    let (period,): (chrono::DateTime<chrono::Utc>,) = sqlx::query_as("select period_start from accounts where id = $1")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ApiError::not_found("That account"))?;
    let amount = micros(b.dollars);
    crate::billing::grant(
        &mut tx,
        id,
        period,
        app.now(),
        "credit",
        Bucket::Credit,
        amount,
        b.reason.trim(),
        ("admin", Some(auth.user.id)),
    )
    .await?;
    sqlx::query("insert into account_overrides (id, account_id, kind, data, reason, actor_kind, actor_id, ends_at) values ($1, $2, 'credit', $3, $4, 'admin', $5, $6)")
        .bind(Uuid::new_v4())
        .bind(id)
        .bind(json!({ "amount_micros": amount }))
        .bind(b.reason.trim())
        .bind(auth.user.id)
        .bind(b.ends_at)
        .execute(&mut *tx)
        .await?;
    events::emit(
        &mut *tx,
        NewEvent::new(
            id,
            "billing",
            "credit.added",
            "success",
            format!("Croncave added a {} credit", crate::billing::dollars(amount)),
        )
        .body(b.reason.trim().to_string())
        .notify(),
    )
    .await?;
    crate::measure::audit(
        &mut *tx,
        id,
        "admin",
        Some(auth.user.id),
        "credit.given",
        "",
        json!({ "amount_micros": amount, "reason": b.reason }),
    )
    .await?;
    crate::billing::maybe_resume(&app, &mut tx, id).await?;
    tx.commit().await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct MoveVersion {
    pub version: i32,
}

/// Move an account to another catalog version (deliberately, never automatically).
pub async fn move_version(
    State(app): State<AppState>,
    Admin(auth): Admin,
    Path(id): Path<Uuid>,
    Json(b): Json<MoveVersion>,
) -> ApiResult<Json<Value>> {
    let c = app.catalog(b.version).await.map_err(|_| ApiError::not_found("That catalog version"))?;
    let (plan,): (Option<String>,) =
        sqlx::query_as("select plan from accounts where id = $1").bind(id).fetch_one(&app.db).await?;
    if let Some(p) = plan
        && c.plan(&p).is_none()
    {
        return Err(ApiError::bad(format!("Version {} has no \"{p}\" plan.", b.version)));
    }
    sqlx::query("update accounts set catalog_version = $2 where id = $1")
        .bind(id)
        .bind(b.version)
        .execute(&app.db)
        .await?;
    crate::measure::audit(
        &app.db,
        id,
        "admin",
        Some(auth.user.id),
        "catalog.account_moved",
        &b.version.to_string(),
        json!({}),
    )
    .await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct Kill {
    pub model: String,
    pub killed: bool,
}

pub async fn ai_kill(State(app): State<AppState>, Admin(_): Admin, Json(b): Json<Kill>) -> ApiResult<Json<Value>> {
    let mut k = app.ai_kill.write().expect("kill switches");
    if b.killed {
        k.insert(b.model);
    } else {
        k.remove(&b.model);
    }
    Ok(Json(json!({ "killed": k.iter().collect::<Vec<_>>() })))
}

/// What the alpha measures, beside what the pricing model assumed.
pub async fn measurements(State(app): State<AppState>, Admin(_): Admin) -> ApiResult<Json<Value>> {
    let wake: Vec<(String, i64, Option<f64>, Option<f64>, Option<i32>)> = sqlx::query_as(
        "select size, count(*), percentile_cont(0.5) within group (order by ms), percentile_cont(0.95) within group (order by ms), max(ms)
         from wake_measurements where ready_at > now() - interval '30 days' group by size order by size",
    )
    .fetch_all(&app.db)
    .await?;
    let (wakes_under_target, wakes_total): (i64, i64) =
        sqlx::query_as("select count(*) filter (where ms < 5000), count(*) from wake_measurements where ready_at > now() - interval '30 days'").fetch_one(&app.db).await?;
    let causes: Vec<(String, f64)> = sqlx::query_as(
        "select cause, sum(seconds) from awake_by_cause where day > current_date - 30 group by cause order by 2 desc",
    )
    .fetch_all(&app.db)
    .await?;
    let usage: Vec<(String, String, i64, i64)> = sqlx::query_as(
        "select u.email, l.bucket, (-sum(l.amount_micros))::bigint, count(*) from ledger_entries l
         join account_members m on m.account_id = l.account_id join users u on u.id = m.user_id
         where l.kind = 'usage' group by 1, 2 order by 1, 2",
    )
    .fetch_all(&app.db)
    .await?;
    let meters: Vec<(String, f64, i64)> =
        sqlx::query_as("select meter, sum(quantity), sum(cost_micros)::bigint from usage_hourly group by 1")
            .fetch_all(&app.db)
            .await?;
    let funnel: Vec<(String, i64)> =
        sqlx::query_as("select kind, count(*) from funnel_events group by 1 order by 2 desc")
            .fetch_all(&app.db)
            .await?;
    let plan_mix: Vec<(Option<String>, i64)> =
        sqlx::query_as("select coalesce(trial_plan, plan), count(*) from accounts group by 1 order by 2 desc")
            .fetch_all(&app.db)
            .await?;
    let (jobs, runs, median_first): (i64, i64, Option<f64>) = sqlx::query_as(
        "select (select count(*) from jobs where status <> 'draft' and kind <> 'dev_server'),
                (select count(*) from runs where trigger <> 'test'),
                (select percentile_cont(0.5) within group (order by (data->>'secs_since_signup')::float) from funnel_events where kind = 'first_result')",
    )
    .fetch_one(&app.db)
    .await?;
    let start_delay: (Option<f64>, Option<f64>) = sqlx::query_as(
        "select percentile_cont(0.5) within group (order by extract(epoch from (started_at - slot_at))),
                percentile_cont(0.95) within group (order by extract(epoch from (started_at - slot_at)))
         from runs where trigger = 'schedule' and started_at is not null",
    )
    .fetch_one(&app.db)
    .await?;
    let total_awake: f64 = causes.iter().map(|c| c.1).sum();
    Ok(Json(json!({
        "wake": {
            "by_size": wake.into_iter().map(|(s, n, p50, p95, max)| json!({ "size": s, "count": n, "p50_ms": p50, "p95_ms": p95, "max_ms": max })).collect::<Vec<_>>(),
            "under_target": wakes_under_target, "total": wakes_total, "target": "under 5 seconds for 95% of wakes",
        },
        "awake_by_cause": causes.into_iter().map(|(c, s)| json!({ "cause": c, "seconds": s, "share": if total_awake > 0.0 { s / total_awake } else { 0.0 } })).collect::<Vec<_>>(),
        "usage_by_account": usage.into_iter().map(|(e, b, m, n)| json!({ "email": e, "bucket": b, "micros": m, "entries": n })).collect::<Vec<_>>(),
        "meters": meters.into_iter().map(|(m, q, c)| json!({ "meter": m, "quantity": q, "cost_micros": c })).collect::<Vec<_>>(),
        "funnel": funnel.into_iter().map(|(k, n)| json!({ "kind": k, "count": n })).collect::<Vec<_>>(),
        "plan_mix": plan_mix.into_iter().map(|(p, n)| json!({ "plan": p.unwrap_or_else(|| "(choosing)".into()), "count": n })).collect::<Vec<_>>(),
        "activation": { "jobs": jobs, "runs": runs, "runs_per_job": if jobs > 0 { runs as f64 / jobs as f64 } else { 0.0 }, "median_secs_to_first_result": median_first },
        "schedule_start_delay": { "p50_secs": start_delay.0, "p95_secs": start_delay.1, "target": "within 15 seconds for 95% of runs" },
        "assumptions": [
            { "what": "Sleep after", "assumed": "30 seconds idle", "measure": "awake_by_cause.idle_grace" },
            { "what": "Stay awake if next job within", "assumed": format!("{} seconds", app.cfg.stay_awake_within_secs), "measure": "awake_by_cause.next_job_soon" },
            { "what": "Wake time", "assumed": "under 5 s", "measure": "wake.p95_ms" },
            { "what": "Markup on compute and disk", "assumed": "25%", "measure": "monthly reconciliation (needs real provider bills)" },
        ],
        "ai_killed": app.ai_kill.read().expect("kill switches").iter().cloned().collect::<Vec<_>>(),
    })))
}
