//! Computers: create, name, size, wake, sleep, restart, reset and delete.

use std::time::Duration;

use axum::Json;
use axum::extract::{Path, State};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::auth::Auth;
use crate::billing::plan_of;
use crate::error::{ApiError, ApiResult};
use crate::events::{self, NewEvent};
use crate::model::Computer;
use crate::providers::compute::ComputerSpec;
use crate::state::AppState;

pub const APPS: &[&str] = &["files", "scripts", "watcher", "code"];

pub async fn list_for(app: &AppState, account: Uuid) -> sqlx::Result<Vec<Value>> {
    let computers: Vec<Computer> =
        sqlx::query_as("select * from computers where account_id = $1 and deleted_at is null order by created_at")
            .bind(account)
            .fetch_all(&app.db)
            .await?;
    let mut out = Vec::new();
    for c in computers {
        out.push(describe(app, &c).await?);
    }
    Ok(out)
}

/// A computer as the web app shows it, with its state in plain words.
pub async fn describe(app: &AppState, c: &Computer) -> sqlx::Result<Value> {
    let (running,): (i64,) = sqlx::query_as("select count(*) from runs where computer_id = $1 and status in ('starting', 'running') and kind <> 'dev_server'")
        .bind(c.id)
        .fetch_one(&app.db)
        .await?;
    let (needs,): (i64,) = sqlx::query_as("select count(*) from approvals a join runs r on r.id = a.run_id where r.computer_id = $1 and a.status = 'pending'")
        .bind(c.id)
        .fetch_one(&app.db)
        .await?;
    let next: Option<(chrono::DateTime<chrono::Utc>, String)> =
        sqlx::query_as("select next_due_at, name from jobs where computer_id = $1 and status = 'active' and next_due_at is not null order by next_due_at limit 1")
            .bind(c.id)
            .fetch_optional(&app.db)
            .await?;
    let ports: Vec<(i32,)> = sqlx::query_as("select port from open_ports where computer_id = $1 order by port")
        .bind(c.id)
        .fetch_all(&app.db)
        .await?;
    let status = if needs > 0 {
        "needs_you"
    } else if running > 0 && c.state == "awake" {
        "working"
    } else {
        c.state.as_str()
    };
    let word = match status {
        "needs_you" => "Needs you",
        "working" => "Working",
        "awake" => "Awake",
        "waking" => "Waking up",
        "sleeping" => "Going to sleep",
        "asleep" => "Asleep",
        "creating" => "Getting ready",
        _ => "Unknown",
    };
    let mut v = serde_json::to_value(c).unwrap_or_default();
    v["status"] = json!(status);
    v["status_word"] = json!(word);
    v["running"] = json!(running);
    v["next_job"] = next.map(|(at, name)| json!({ "at": at, "name": name })).unwrap_or(Value::Null);
    v["open_ports"] = json!(ports.into_iter().map(|(p,)| p).collect::<Vec<_>>());
    Ok(v)
}

pub async fn load(app: &AppState, auth: &Auth, id: Uuid) -> ApiResult<Computer> {
    sqlx::query_as("select * from computers where id = $1 and account_id = $2 and deleted_at is null")
        .bind(id)
        .bind(auth.account.id)
        .fetch_optional(&app.db)
        .await?
        .ok_or_else(|| ApiError::not_found("That computer"))
}

#[derive(Deserialize)]
pub struct NewComputer {
    pub name: String,
    pub size: String,
    #[serde(default)]
    pub disk_gb: Option<i32>,
    #[serde(default)]
    pub apps: Option<Vec<String>>,
    #[serde(default)]
    pub sleep_delay_secs: Option<i32>,
    #[serde(default)]
    pub wake_for_schedule: Option<bool>,
}

pub async fn create(State(app): State<AppState>, auth: Auth, Json(b): Json<NewComputer>) -> ApiResult<Json<Value>> {
    auth.require_ready()?;
    let (catalog, plan) = plan_of(&app, &auth.account).await?;
    let name = b.name.trim();
    if name.is_empty() || name.len() > 40 {
        return Err(ApiError::bad("Give your computer a name (up to 40 characters)."));
    }
    let size = catalog.size(&b.size).cloned().ok_or_else(|| ApiError::bad("Choose Small, Medium or Large."))?;
    if !catalog.size_allowed(&plan, &b.size) {
        return Err(ApiError::limit(format!("{} computers need a bigger plan than {}.", size.label, plan.name)));
    }
    let (count,): (i64,) =
        sqlx::query_as("select count(*) from computers where account_id = $1 and deleted_at is null")
            .bind(auth.account.id)
            .fetch_one(&app.db)
            .await?;
    if count >= plan.computers {
        return Err(ApiError::limit(format!(
            "{} includes {} computer{}. Delete one or choose a bigger plan.",
            plan.name,
            plan.computers,
            if plan.computers == 1 { "" } else { "s" }
        )));
    }
    let disk_gb = b.disk_gb.unwrap_or(size.disk_gb).clamp(5, size.disk_gb * 4);
    let sleep_delay = b.sleep_delay_secs.unwrap_or(30);
    if !(30..=86_400).contains(&sleep_delay) {
        return Err(ApiError::bad("Choose a sleep delay between 30 seconds and a day."));
    }
    let id = Uuid::new_v4();
    let spec = ComputerSpec { id, cpu: size.cpu, memory_gb: size.memory_gb, disk_gb };
    let compute_ref = app
        .providers
        .compute
        .create(&spec)
        .await
        .map_err(|e| ApiError::unavailable(format!("Couldn't create the computer: {e}")))?;
    let mut tx = app.db.begin().await?;
    sqlx::query(
        "insert into computers (id, account_id, name, size, cpu, memory_gb, disk_gb, state, compute_ref, wake_requested_at, wake_cause, time_zone,
                                sleep_delay_secs, wake_for_schedule)
         values ($1, $2, $3, $4, $5, $6, $7, 'asleep', $8, $9, 'created', $10, $11, $12)",
    )
    .bind(id)
    .bind(auth.account.id)
    .bind(name)
    .bind(&b.size)
    .bind(size.cpu)
    .bind(size.memory_gb)
    .bind(disk_gb)
    .bind(&compute_ref)
    .bind(app.now())
    .bind(&auth.user.time_zone)
    .bind(sleep_delay)
    .bind(b.wake_for_schedule.unwrap_or(true))
    .execute(&mut *tx)
    .await?;
    let apps = b.apps.unwrap_or_else(|| APPS.iter().map(|s| s.to_string()).collect());
    for a in APPS.iter().filter(|a| **a == "files" || apps.iter().any(|x| x == *a)) {
        sqlx::query("insert into installs (id, computer_id, app_id, version) values ($1, $2, $3, '1')")
            .bind(Uuid::new_v4())
            .bind(id)
            .bind(a)
            .execute(&mut *tx)
            .await?;
    }
    events::emit(
        &mut *tx,
        NewEvent::new(auth.account.id, "platform", "computer.created", "success", format!("Created {name}"))
            .computer(id)
            .actor(auth.actor_kind, Some(auth.user.id)),
    )
    .await?;
    crate::measure::audit(
        &mut *tx,
        auth.account.id,
        auth.actor_kind,
        Some(auth.user.id),
        "computer.created",
        name,
        json!({ "size": b.size }),
    )
    .await?;
    tx.commit().await?;
    crate::measure::funnel(&app.db, Some(auth.account.id), "computer_created", json!({ "size": b.size })).await;
    app.kick.notify_waiters();
    let c: Computer = sqlx::query_as("select * from computers where id = $1").bind(id).fetch_one(&app.db).await?;
    Ok(Json(describe(&app, &c).await?))
}

pub async fn get(State(app): State<AppState>, auth: Auth, Path(id): Path<Uuid>) -> ApiResult<Json<Value>> {
    let c = load(&app, &auth, id).await?;
    let catalog = app.catalog(auth.account.catalog_version).await?;
    let mut v = describe(&app, &c).await?;
    v["costs"] = json!({
        "awake_hourly_micros": catalog.awake_hourly_micros(&c.size),
        "disk_monthly_micros": catalog.disk_monthly_micros(c.disk_gb),
        "keep_awake_monthly_micros": catalog.awake_hourly_micros(&c.size) * 24 * 30,
    });
    let installs: Vec<(String,)> =
        sqlx::query_as("select app_id from installs where computer_id = $1 order by created_at")
            .bind(id)
            .fetch_all(&app.db)
            .await?;
    v["apps"] = json!(installs.into_iter().map(|(a,)| a).collect::<Vec<_>>());
    let wakes: Vec<(String, i32, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        "select cause, ms, ready_at from wake_measurements where computer_id = $1 order by id desc limit 10",
    )
    .bind(id)
    .fetch_all(&app.db)
    .await?;
    v["recent_wakes"] =
        json!(wakes.into_iter().map(|(c, ms, at)| json!({ "cause": c, "ms": ms, "at": at })).collect::<Vec<_>>());
    Ok(Json(v))
}

#[derive(Deserialize)]
pub struct UpdateComputer {
    pub name: Option<String>,
    pub sleep_delay_secs: Option<i32>,
    pub keep_awake: Option<bool>,
    pub size: Option<String>,
    pub disk_gb: Option<i32>,
    pub wake_for_schedule: Option<bool>,
    pub time_zone: Option<String>,
}

pub async fn update(
    State(app): State<AppState>,
    auth: Auth,
    Path(id): Path<Uuid>,
    Json(b): Json<UpdateComputer>,
) -> ApiResult<Json<Value>> {
    let c = load(&app, &auth, id).await?;
    let (catalog, plan) = plan_of(&app, &auth.account).await?;
    let mut tx = app.db.begin().await?;
    if let Some(n) = &b.name {
        let n = n.trim();
        if n.is_empty() || n.len() > 40 {
            return Err(ApiError::bad("Give your computer a name (up to 40 characters)."));
        }
        sqlx::query("update computers set name = $2 where id = $1").bind(id).bind(n).execute(&mut *tx).await?;
    }
    if let Some(d) = b.sleep_delay_secs {
        if !(30..=86_400).contains(&d) {
            return Err(ApiError::bad("Choose a sleep delay between 30 seconds and a day."));
        }
        sqlx::query("update computers set sleep_delay_secs = $2 where id = $1")
            .bind(id)
            .bind(d)
            .execute(&mut *tx)
            .await?;
    }
    if let Some(w) = b.wake_for_schedule {
        sqlx::query("update computers set wake_for_schedule = $2 where id = $1")
            .bind(id)
            .bind(w)
            .execute(&mut *tx)
            .await?;
    }
    if let Some(z) = &b.time_zone {
        if crate::zones::find(z).is_none() {
            return Err(ApiError::bad("Choose one of the US time zones."));
        }
        // The computer's jobs follow it, and their next runs move to the new zone's clock.
        sqlx::query("update computers set time_zone = $2 where id = $1").bind(id).bind(z).execute(&mut *tx).await?;
        let jobs: Vec<(Uuid, Option<String>, String)> =
            sqlx::query_as("select id, schedule, status from jobs where computer_id = $1 and status <> 'deleted'")
                .bind(id)
                .fetch_all(&mut *tx)
                .await?;
        for (job, schedule, status) in jobs {
            let next = if status == "active" {
                schedule.as_deref().and_then(|s| crate::jobs::next_after(s, z, app.now()))
            } else {
                None
            };
            sqlx::query("update jobs set time_zone = $2, next_due_at = $3 where id = $1")
                .bind(job)
                .bind(z)
                .bind(next)
                .execute(&mut *tx)
                .await?;
        }
    }
    if let Some(k) = b.keep_awake {
        if k {
            let (n,): (i64,) = sqlx::query_as("select count(*) from computers where account_id = $1 and keep_awake and deleted_at is null and id <> $2")
                .bind(auth.account.id)
                .bind(id)
                .fetch_one(&mut *tx)
                .await?;
            if n >= plan.keep_awake {
                return Err(ApiError::limit(if plan.keep_awake == 0 {
                    format!("{} doesn't include keep-awake computers. Pro and Max do.", plan.name)
                } else {
                    format!("{} includes {} keep-awake computer(s).", plan.name, plan.keep_awake)
                }));
            }
        }
        sqlx::query("update computers set keep_awake = $2 where id = $1").bind(id).bind(k).execute(&mut *tx).await?;
    }
    if b.size.is_some() || b.disk_gb.is_some() {
        if c.state != "asleep" {
            return Err(ApiError::conflict("A computer changes size while it's asleep. Put it to sleep first."));
        }
        let size_id = b.size.clone().unwrap_or(c.size.clone());
        let size = catalog.size(&size_id).ok_or_else(|| ApiError::bad("Choose Small, Medium or Large."))?;
        if !catalog.size_allowed(&plan, &size_id) {
            return Err(ApiError::limit(format!("{} computers need a bigger plan than {}.", size.label, plan.name)));
        }
        let disk = b.disk_gb.unwrap_or(c.disk_gb.max(size.disk_gb));
        if disk < c.disk_gb {
            return Err(ApiError::bad("Storage can grow but not shrink."));
        }
        sqlx::query("update computers set size = $2, cpu = $3, memory_gb = $4, disk_gb = $5 where id = $1")
            .bind(id)
            .bind(&size_id)
            .bind(size.cpu)
            .bind(size.memory_gb)
            .bind(disk.min(size.disk_gb * 4))
            .execute(&mut *tx)
            .await?;
    }
    crate::measure::audit(
        &mut *tx,
        auth.account.id,
        auth.actor_kind,
        Some(auth.user.id),
        "computer.changed",
        &c.name,
        json!({}),
    )
    .await?;
    events::live(&mut *tx, auth.account.id, "computer", &id.to_string(), json!({})).await?;
    tx.commit().await?;
    app.kick.notify_waiters();
    let c = load(&app, &auth, id).await?;
    Ok(Json(describe(&app, &c).await?))
}

/// Ask for a computer to wake. Checks the cap and the plan's awake-at-once limit.
pub async fn request_wake(app: &AppState, c: &Computer, cause: &str) -> ApiResult<()> {
    if matches!(c.state.as_str(), "awake" | "waking") {
        return Ok(());
    }
    let account: crate::model::Account =
        sqlx::query_as("select * from accounts where id = $1").bind(c.account_id).fetch_one(&app.db).await?;
    if account.paused_at.is_some() {
        return Err(ApiError::limit(
            "Work is paused at your spending cap. Raise the cap in Plans and billing to wake your computer.",
        )
        .with_code("paused"));
    }
    let (catalog, plan) = plan_of(app, &account).await?;
    if !catalog.size_allowed(&plan, &c.size) {
        return Err(ApiError::limit(format!(
            "{} computers need a bigger plan than {}. Resize it or change plan.",
            catalog.size(&c.size).map(|s| s.label.as_str()).unwrap_or("This size"),
            plan.name
        )));
    }
    let awake: Vec<(String,)> = sqlx::query_as("select name from computers where account_id = $1 and id <> $2 and deleted_at is null and state in ('awake', 'waking', 'sleeping')")
        .bind(c.account_id)
        .bind(c.id)
        .fetch_all(&app.db)
        .await?;
    if awake.len() as i64 >= plan.awake_at_once {
        let names: Vec<String> = awake.into_iter().map(|(n,)| n).collect();
        return Err(ApiError::limit(format!(
            "{} lets {} computer{} be awake at a time, and {} {} awake now. It'll wake once that one sleeps.",
            plan.name,
            plan.awake_at_once,
            if plan.awake_at_once == 1 { "" } else { "s" },
            names.join(", "),
            if names.len() == 1 { "is" } else { "are" }
        ))
        .with_code("awake_limit"));
    }
    sqlx::query("update computers set wake_requested_at = coalesce(wake_requested_at, $2), wake_cause = coalesce(wake_cause, $3) where id = $1")
        .bind(c.id)
        .bind(app.now())
        .bind(cause)
        .execute(&app.db)
        .await?;
    app.kick.notify_waiters();
    Ok(())
}

/// Wake a computer and wait until its agent is connected.
pub async fn ensure_awake(app: &AppState, c: &Computer, cause: &str) -> ApiResult<()> {
    if c.state == "awake" && app.relay.is_connected(c.id).await {
        return Ok(());
    }
    request_wake(app, c, cause).await?;
    for _ in 0..200 {
        if app.relay.is_connected(c.id).await {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    Err(ApiError::unavailable("Your computer is taking longer than usual to wake. Try again in a moment."))
}

#[derive(Deserialize)]
pub struct PresenceBody {
    /// app, files, preview
    pub cause: String,
    pub app: Option<String>,
}

/// The web app reports what the person has open; that keeps the computer awake.
pub async fn presence(
    State(app): State<AppState>,
    auth: Auth,
    Path(id): Path<Uuid>,
    Json(b): Json<PresenceBody>,
) -> ApiResult<Json<Value>> {
    let c = load(&app, &auth, id).await?;
    if !["app", "files", "preview"].contains(&b.cause.as_str()) {
        return Err(ApiError::bad("unknown presence"));
    }
    touch_presence(&app, c.id, auth.user.id, &b.cause, b.app.as_deref()).await?;
    let wake = request_wake(&app, &c, &format!("open:{}", b.app.as_deref().unwrap_or(&b.cause))).await;
    let c = load(&app, &auth, id).await?;
    let mut v = describe(&app, &c).await?;
    if let Err(e) = wake {
        v["wake_error"] = json!(e.message);
    }
    Ok(Json(v))
}

pub async fn touch_presence(
    app: &AppState,
    computer: Uuid,
    user: Uuid,
    cause: &str,
    app_name: Option<&str>,
) -> sqlx::Result<()> {
    sqlx::query(
        "insert into presence (computer_id, user_id, cause, app, until_at) values ($1, $2, $3, $4, $5)
         on conflict (computer_id, user_id, cause) do update set until_at = excluded.until_at, app = excluded.app",
    )
    .bind(computer)
    .bind(user)
    .bind(cause)
    .bind(app_name)
    .bind(app.now() + chrono::Duration::seconds(40))
    .execute(&app.db)
    .await?;
    Ok(())
}

#[derive(Deserialize)]
pub struct ActionBody {
    pub action: String,
}

/// wake, sleep, restart, reset.
pub async fn action(
    State(app): State<AppState>,
    auth: Auth,
    Path(id): Path<Uuid>,
    Json(b): Json<ActionBody>,
) -> ApiResult<Json<Value>> {
    let c = load(&app, &auth, id).await?;
    match b.action.as_str() {
        "wake" => request_wake(&app, &c, "user").await?,
        "sleep" => {
            sqlx::query("delete from presence where computer_id = $1").bind(id).execute(&app.db).await?;
            let (running,): (i64,) = sqlx::query_as("select count(*) from runs where computer_id = $1 and status in ('starting', 'running') and kind <> 'dev_server'").bind(id).fetch_one(&app.db).await?;
            if running > 0 {
                return Err(ApiError::conflict(
                    "Something is running. Stop it first, or let it finish and the computer sleeps by itself.",
                ));
            }
            crate::orchestrator::sleep(&app, id, "user").await?;
        }
        "restart" => {
            crate::orchestrator::sleep(&app, id, "restart").await?;
            let c = load(&app, &auth, id).await?;
            request_wake(&app, &c, "restart").await?;
        }
        "reset" => {
            crate::orchestrator::sleep(&app, id, "reset").await?;
            let compute_ref = c.compute_ref.clone().unwrap_or_default();
            app.providers.compute.destroy(&compute_ref).await.map_err(|e| ApiError::unavailable(e.to_string()))?;
            let spec = ComputerSpec { id, cpu: c.cpu, memory_gb: c.memory_gb, disk_gb: c.disk_gb };
            let new_ref =
                app.providers.compute.create(&spec).await.map_err(|e| ApiError::unavailable(e.to_string()))?;
            sqlx::query("update computers set compute_ref = $2, agent_epoch = null, agent_acked_seq = 0, health = '{}' where id = $1").bind(id).bind(new_ref).execute(&app.db).await?;
            sqlx::query("insert into file_records (computer_id, path, change, actor_kind, actor_id) values ($1, '', 'reset', 'user', $2)").bind(id).bind(auth.user.id).execute(&app.db).await?;
            events::emit(
                &app.db,
                NewEvent::new(auth.account.id, "platform", "computer.reset", "warning", format!("Reset {}", c.name))
                    .body("Its disk was replaced with an empty one.")
                    .computer(id)
                    .actor(auth.actor_kind, Some(auth.user.id)),
            )
            .await?;
        }
        _ => return Err(ApiError::bad("Unknown action.")),
    }
    crate::measure::audit(
        &app.db,
        auth.account.id,
        auth.actor_kind,
        Some(auth.user.id),
        &format!("computer.{}", b.action),
        &c.name,
        json!({}),
    )
    .await?;
    let c = load(&app, &auth, id).await?;
    Ok(Json(describe(&app, &c).await?))
}

pub async fn delete(State(app): State<AppState>, auth: Auth, Path(id): Path<Uuid>) -> ApiResult<Json<Value>> {
    let c = load(&app, &auth, id).await?;
    crate::orchestrator::sleep(&app, id, "delete").await?;
    if let Some(r) = &c.compute_ref {
        app.providers.compute.destroy(r).await.map_err(|e| ApiError::unavailable(e.to_string()))?;
    }
    let mut tx = app.db.begin().await?;
    let now = app.now();
    sqlx::query("update computers set deleted_at = $2, state = 'deleted' where id = $1")
        .bind(id)
        .bind(now)
        .execute(&mut *tx)
        .await?;
    sqlx::query("update jobs set status = 'deleted', next_due_at = null where computer_id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("update runs set status = 'stopped', status_note = 'The computer was deleted', ended_at = $2 where computer_id = $1 and status in ('queued', 'waiting', 'starting', 'running', 'held')").bind(id).bind(now).execute(&mut *tx).await?;
    events::emit(
        &mut *tx,
        NewEvent::new(auth.account.id, "platform", "computer.deleted", "info", format!("Deleted {}", c.name))
            .actor(auth.actor_kind, Some(auth.user.id)),
    )
    .await?;
    crate::measure::audit(
        &mut *tx,
        auth.account.id,
        auth.actor_kind,
        Some(auth.user.id),
        "computer.deleted",
        &c.name,
        json!({}),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(json!({ "ok": true })))
}
