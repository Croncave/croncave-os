//! Home: what happened since you left, what needs you, recent results and what's next.

use axum::Json;
use axum::extract::State;
use serde_json::{Value, json};

use crate::auth::Auth;
use crate::error::ApiResult;
use crate::state::AppState;

pub async fn home(State(app): State<AppState>, auth: Auth) -> ApiResult<Json<Value>> {
    let account = auth.account.id;
    let since = auth.user.previous_seen_at.unwrap_or(auth.user.created_at);
    let happened: Vec<crate::events::EventRow> = sqlx::query_as(
        "select id, computer_id, actor_kind, app, kind, level, title, body, data, run_id, job_id, read_at, created_at from events
         where account_id = $1 and created_at > $2 and level <> 'system' order by id desc limit 20",
    )
    .bind(account)
    .bind(since)
    .fetch_all(&app.db)
    .await?;
    let counts: Vec<(String, i64)> = sqlx::query_as(
        "select level, count(*) from events where account_id = $1 and created_at > $2 and level <> 'system' group by 1",
    )
    .bind(account)
    .bind(since)
    .fetch_all(&app.db)
    .await?;
    let approvals: Vec<(uuid::Uuid, String, String, String, uuid::Uuid)> = sqlx::query_as(
        "select r.id, a.request_id, a.command, j.name, r.computer_id from approvals a join runs r on r.id = a.run_id join jobs j on j.id = r.job_id
         where r.account_id = $1 and a.status = 'pending' and r.status = 'running' order by a.created_at",
    )
    .bind(account)
    .fetch_all(&app.db)
    .await?;
    let reviews: Vec<(uuid::Uuid, String, uuid::Uuid)> = sqlx::query_as(
        "select e.run_id, e.title, e.computer_id from events e where e.account_id = $1 and e.kind = 'agent.review_ready' and e.read_at is null order by e.id desc limit 5",
    )
    .bind(account)
    .fetch_all(&app.db)
    .await?;
    let failing: Vec<(uuid::Uuid, uuid::Uuid, String, String, Option<String>, Option<String>, uuid::Uuid)> = sqlx::query_as(
        "select j.id, r.id, j.name, j.app, r.error_plain, r.error_fix, j.computer_id from jobs j
         join lateral (select * from runs where job_id = j.id and trigger <> 'test' order by queued_at desc limit 1) r on true
         where j.account_id = $1 and j.status = 'active' and r.status in ('failed', 'timed_out')",
    )
    .bind(account)
    .fetch_all(&app.db)
    .await?;
    let results: Vec<(uuid::Uuid, String, String, String, Option<String>, Option<chrono::DateTime<chrono::Utc>>, uuid::Uuid)> = sqlx::query_as(
        "select r.id, j.name, j.app, r.status, r.headline, r.ended_at, r.computer_id from runs r join jobs j on j.id = r.job_id
         where r.account_id = $1 and r.ended_at is not null and r.trigger <> 'test' and r.kind <> 'dev_server' order by r.ended_at desc limit 8",
    )
    .bind(account)
    .fetch_all(&app.db)
    .await?;
    let upcoming: Vec<(uuid::Uuid, String, String, chrono::DateTime<chrono::Utc>, uuid::Uuid)> = sqlx::query_as(
        "select id, name, app, next_due_at, computer_id from jobs where account_id = $1 and status = 'active' and next_due_at is not null order by next_due_at limit 5",
    )
    .bind(account)
    .fetch_all(&app.db)
    .await?;
    // Work in progress, for "still running" rows with their progress.
    let running: Vec<(uuid::Uuid, String, String, Option<chrono::DateTime<chrono::Utc>>, Option<Value>, uuid::Uuid)> = sqlx::query_as(
        "select r.id, j.name, j.app, r.started_at, r.progress, r.computer_id from runs r join jobs j on j.id = r.job_id
         where r.account_id = $1 and r.status = 'running' and r.trigger <> 'test' and r.kind <> 'dev_server' order by r.started_at",
    )
    .bind(account)
    .fetch_all(&app.db)
    .await?;
    // What each computer has switched on, for "1 job running, 4 watches on".
    let active: Vec<(uuid::Uuid, String, i64)> = sqlx::query_as(
        "select computer_id, app, count(*) from jobs where account_id = $1 and status = 'active' and kind <> 'dev_server' group by 1, 2",
    )
    .bind(account)
    .fetch_all(&app.db)
    .await?;
    Ok(Json(json!({
        "name": auth.user.name,
        "running": running.into_iter().map(|(id, name, a, at, progress, c)| json!({ "run_id": id, "job": name, "app": a, "started_at": at, "progress": progress, "computer_id": c })).collect::<Vec<_>>(),
        "active": active.into_iter().map(|(c, a, n)| json!({ "computer_id": c, "app": a, "count": n })).collect::<Vec<_>>(),
        "since": since,
        "counts": counts.into_iter().map(|(l, n)| (l, json!(n))).collect::<serde_json::Map<_, _>>(),
        "happened": happened,
        "needs_you": {
            "approvals": approvals.into_iter().map(|(run, req, cmd, job, c)| json!({ "run_id": run, "request_id": req, "command": cmd, "job": job, "computer_id": c })).collect::<Vec<_>>(),
            "reviews": reviews.into_iter().map(|(run, title, c)| json!({ "run_id": run, "title": title, "computer_id": c })).collect::<Vec<_>>(),
            "failing": failing.into_iter().map(|(job, run, name, a, plain, fix, c)| json!({ "job_id": job, "run_id": run, "name": name, "app": a, "why": plain, "fix": fix, "computer_id": c })).collect::<Vec<_>>(),
        },
        "results": results.into_iter().map(|(id, name, a, s, h, at, c)| json!({ "run_id": id, "job": name, "app": a, "status": s, "headline": h, "at": at, "computer_id": c })).collect::<Vec<_>>(),
        "upcoming": upcoming.into_iter().map(|(id, name, a, at, c)| json!({ "job_id": id, "name": name, "app": a, "at": at, "computer_id": c })).collect::<Vec<_>>(),
    })))
}
