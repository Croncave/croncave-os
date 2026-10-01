//! The orchestrator: wakes computers for work, puts them to sleep about 30 seconds after
//! nothing is active (unless their next job is due soon), meters awake time, and records
//! why each computer was awake.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

use crate::error::{ApiError, ApiResult};
use crate::events::{self, NewEvent};
use crate::model::Computer;
use crate::providers::compute::{Boot, ComputerSpec, DriverStatus};
use crate::state::AppState;

pub async fn run_loop(app: AppState) {
    let sleeping: Arc<Mutex<HashSet<Uuid>>> = Arc::default();
    let mut last = Instant::now();
    loop {
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(1)) => {}
            _ = app.kick.notified() => {}
        }
        // Billed time is measured with a monotonic clock, never the adjustable one.
        let dt = last.elapsed().as_secs_f64().min(5.0);
        last = Instant::now();
        if let Err(e) = tick(&app, dt, &sleeping).await {
            tracing::error!(error = %e, "orchestrator tick failed");
        }
    }
}

async fn tick(app: &AppState, dt: f64, sleeping: &Arc<Mutex<HashSet<Uuid>>>) -> anyhow::Result<()> {
    let now = app.now();
    sqlx::query("update computers set unmetered_disk_secs = unmetered_disk_secs + $1 where deleted_at is null")
        .bind(dt)
        .execute(&app.db)
        .await?;
    sqlx::query("update computers set unmetered_awake_secs = unmetered_awake_secs + $1 where deleted_at is null and state in ('awake', 'waking', 'sleeping')")
        .bind(dt)
        .execute(&app.db)
        .await?;
    sqlx::query("delete from presence where until_at < $1").bind(now).execute(&app.db).await?;

    let computers: Vec<Computer> =
        sqlx::query_as("select * from computers where deleted_at is null").fetch_all(&app.db).await?;
    for c in computers {
        if sleeping.lock().expect("set").contains(&c.id) {
            continue;
        }
        match c.state.as_str() {
            "asleep" if c.wake_requested_at.is_some() => {
                if let Err(e) = start(app, &c).await {
                    tracing::warn!(computer = %c.id, error = %e.message, "wake failed");
                }
            }
            "waking" => {
                if app.relay.is_connected(c.id).await {
                    mark_awake(app, c.id).await?;
                } else if (now - c.state_changed_at).num_seconds() > 30 {
                    let status = app.providers.compute.status(c.compute_ref.as_deref().unwrap_or_default()).await;
                    tracing::warn!(computer = %c.id, ?status, "computer did not connect in time; retrying");
                    stop_quietly(app, &c).await;
                    sqlx::query("update computers set state = 'asleep', state_changed_at = $2, note = 'It took too long to wake; trying again.' where id = $1")
                        .bind(c.id)
                        .bind(now)
                        .execute(&app.db)
                        .await?;
                }
            }
            "awake" => awake_tick(app, &c, dt, sleeping).await?,
            _ => {}
        }
    }
    Ok(())
}

/// What keeps a computer awake right now.
pub async fn active_causes(app: &AppState, c: &Computer) -> anyhow::Result<Vec<String>> {
    let mut causes: Vec<String> = sqlx::query_scalar(
        "select distinct case when kind = 'agent_task' then 'agent_session' else 'run' end from runs
         where computer_id = $1 and status in ('queued', 'waiting', 'starting', 'running') and kind <> 'dev_server'",
    )
    .bind(c.id)
    .fetch_all(&app.db)
    .await?;
    let presence: Vec<String> =
        sqlx::query_scalar("select distinct cause from presence where computer_id = $1 and until_at >= $2")
            .bind(c.id)
            .bind(app.now())
            .fetch_all(&app.db)
            .await?;
    causes.extend(presence.into_iter().map(|p| if p == "app" { "open_app".into() } else { p }));
    if c.keep_awake {
        causes.push("keep_awake".into());
    }
    Ok(causes)
}

async fn awake_tick(app: &AppState, c: &Computer, dt: f64, sleeping: &Arc<Mutex<HashSet<Uuid>>>) -> anyhow::Result<()> {
    let now = app.now();
    if !app.relay.is_connected(c.id).await && (now - c.state_changed_at).num_seconds() > 15 {
        let status = app.providers.compute.status(c.compute_ref.as_deref().unwrap_or_default()).await;
        if !matches!(status, Ok(DriverStatus::Running)) {
            // The computer stopped by itself: record it and let the next wake start it fresh.
            sqlx::query("update computers set state = 'asleep', state_changed_at = $2, connected = false, note = 'It stopped unexpectedly and was put to sleep.' where id = $1")
                .bind(c.id)
                .bind(now)
                .execute(&app.db)
                .await?;
            fail_lost_runs(app, c.id, "The computer stopped unexpectedly during this run.").await?;
            events::emit(
                &app.db,
                NewEvent::new(
                    c.account_id,
                    "platform",
                    "computer.crashed",
                    "warning",
                    format!("{} stopped unexpectedly", c.name),
                )
                .computer(c.id),
            )
            .await?;
            return Ok(());
        }
    }
    let causes = active_causes(app, c).await?;
    let paused = crate::billing::is_paused(&app.db, c.account_id).await?;
    let attribute = |cause: String| {
        let app = app.clone();
        let (cid, aid) = (c.id, c.account_id);
        async move {
            let _ = sqlx::query(
                "insert into awake_by_cause (computer_id, account_id, day, cause, seconds) values ($1, $2, $3, $4, $5)
                 on conflict (computer_id, day, cause) do update set seconds = awake_by_cause.seconds + excluded.seconds",
            )
            .bind(cid)
            .bind(aid)
            .bind(app.now().date_naive())
            .bind(cause)
            .bind(dt)
            .execute(&app.db)
            .await;
        }
    };
    let running_work = causes.iter().any(|c| c == "run" || c == "agent_session");
    if paused && !running_work {
        attribute("cap_wind_down".into()).await;
        spawn_sleep(app, c.id, "cap", sleeping);
        return Ok(());
    }
    if !causes.is_empty() {
        sqlx::query("update computers set last_active_at = $2 where id = $1")
            .bind(c.id)
            .bind(now)
            .execute(&app.db)
            .await?;
        let share = causes.len();
        for cause in causes {
            let app = app.clone();
            let _ = sqlx::query(
                "insert into awake_by_cause (computer_id, account_id, day, cause, seconds) values ($1, $2, $3, $4, $5)
                 on conflict (computer_id, day, cause) do update set seconds = awake_by_cause.seconds + excluded.seconds",
            )
            .bind(c.id)
            .bind(c.account_id)
            .bind(now.date_naive())
            .bind(cause)
            .bind(dt / share as f64)
            .execute(&app.db)
            .await;
        }
        return Ok(());
    }
    let idle = (now - c.last_active_at).num_milliseconds() as f64 / 1000.0;
    if idle < c.sleep_delay_secs as f64 {
        attribute("idle_grace".into()).await;
        return Ok(());
    }
    // Waking again soon would cost more than staying up.
    let (soon,): (bool,) = sqlx::query_as(
        "select exists(select 1 from jobs where computer_id = $1 and status = 'active' and next_due_at is not null and next_due_at <= $2)",
    )
    .bind(c.id)
    .bind(now + chrono::Duration::seconds(app.cfg.stay_awake_within_secs))
    .fetch_one(&app.db)
    .await?;
    if soon {
        attribute("next_job_soon".into()).await;
        return Ok(());
    }
    spawn_sleep(app, c.id, "idle", sleeping);
    Ok(())
}

fn spawn_sleep(app: &AppState, id: Uuid, cause: &'static str, sleeping: &Arc<Mutex<HashSet<Uuid>>>) {
    if !sleeping.lock().expect("set").insert(id) {
        return;
    }
    let (app, set) = (app.clone(), sleeping.clone());
    tokio::spawn(async move {
        if let Err(e) = sleep(&app, id, cause).await {
            tracing::warn!(computer = %id, error = %e.message, "could not put the computer to sleep");
        }
        set.lock().expect("set").remove(&id);
    });
}

/// Boot a computer: a one-time bootstrap token, then the driver.
async fn start(app: &AppState, c: &Computer) -> ApiResult<()> {
    // Re-check limits at wake time (plans change, other computers wake).
    if let Err(e) = crate::computers::request_wake(
        app,
        &Computer { state: "asleep".into(), ..c.clone() },
        c.wake_cause.as_deref().unwrap_or("wake"),
    )
    .await
    {
        sqlx::query("update computers set wake_requested_at = null, wake_cause = null, note = $2 where id = $1")
            .bind(c.id)
            .bind(&e.message)
            .execute(&app.db)
            .await?;
        events::live(&app.db, c.account_id, "computer", &c.id.to_string(), json!({})).await?;
        return Err(e);
    }
    let token = crate::crypto::token();
    let now = app.now();
    sqlx::query("update computers set state = 'waking', state_changed_at = $2, bootstrap_hash = $3, bootstrap_expires_at = $4, connected = false, note = null where id = $1")
        .bind(c.id)
        .bind(now)
        .bind(crate::crypto::hash(&token))
        .bind(app.real_now() + chrono::Duration::minutes(2))
        .execute(&app.db)
        .await?;
    events::live(&app.db, c.account_id, "computer", &c.id.to_string(), json!({ "state": "waking" })).await?;
    let spec = ComputerSpec { id: c.id, cpu: c.cpu, memory_gb: c.memory_gb, disk_gb: c.disk_gb };
    let boot = Boot {
        relay_url: app.cfg.relay_url.clone(),
        bootstrap_token: token,
        env: vec![("CRONCAVE_DEMO_URL".into(), app.cfg.demo_url.clone())],
    };
    if let Err(e) = app.providers.compute.start(c.compute_ref.as_deref().unwrap_or_default(), &spec, &boot).await {
        sqlx::query("update computers set state = 'asleep', state_changed_at = $2, wake_requested_at = null, note = $3 where id = $1")
            .bind(c.id)
            .bind(now)
            .bind(format!("Couldn't wake: {e}"))
            .execute(&app.db)
            .await?;
        events::emit(
            &app.db,
            NewEvent::new(
                c.account_id,
                "platform",
                "computer.wake_failed",
                "failed",
                format!("Couldn't wake {}", c.name),
            )
            .body(e.to_string())
            .computer(c.id)
            .notify(),
        )
        .await?;
        return Err(ApiError::unavailable(e.to_string()));
    }
    Ok(())
}

/// The agent connected: the computer is ready. Records the wake time.
pub async fn mark_awake(app: &AppState, id: Uuid) -> anyhow::Result<()> {
    let now = app.now();
    let row: Option<(Uuid, String, String, Option<chrono::DateTime<Utc>>, Option<String>, chrono::DateTime<Utc>)> = sqlx::query_as(
        "with old as (select id, wake_requested_at, wake_cause, state_changed_at from computers where id = $1 and state = 'waking' for update)
         update computers c set state = 'awake', state_changed_at = $2, last_active_at = $2, wake_requested_at = null, wake_cause = null
         from old where c.id = old.id
         returning c.account_id, c.name, c.size, old.wake_requested_at, old.wake_cause, old.state_changed_at",
    )
    .bind(id)
    .bind(now)
    .fetch_optional(&app.db)
    .await?;
    let Some((account, name, size, requested, cause, waking_since)) = row else { return Ok(()) };
    let cause = cause.unwrap_or_else(|| "wake".into());
    let requested = requested.unwrap_or(waking_since);
    record_wake(app, id, requested, &cause).await?;
    let ms = (now - requested).num_milliseconds();
    tracing::info!(computer = %id, ms, cause, "computer awake");
    events::emit(
        &app.db,
        NewEvent::new(account, "platform", "computer.woke", "system", format!("{name} woke up"))
            .computer(id)
            .data(json!({ "cause": cause, "size": size, "ms": ms })),
    )
    .await?;
    events::live(&app.db, account, "computer", &id.to_string(), json!({ "state": "awake" })).await?;
    app.kick.notify_waiters();
    Ok(())
}

/// Put a computer to sleep: tell the agent, close the connection, stop it, keep the disk.
pub async fn sleep(app: &AppState, id: Uuid, cause: &str) -> ApiResult<()> {
    let c: Computer = sqlx::query_as("select * from computers where id = $1").bind(id).fetch_one(&app.db).await?;
    if c.state == "asleep" {
        sqlx::query("update computers set wake_requested_at = null where id = $1").bind(id).execute(&app.db).await?;
        return Ok(());
    }
    let now = app.now();
    sqlx::query("update computers set state = 'sleeping', state_changed_at = $2 where id = $1")
        .bind(id)
        .bind(now)
        .execute(&app.db)
        .await?;
    events::live(&app.db, c.account_id, "computer", &id.to_string(), json!({ "state": "sleeping" })).await?;
    let _ = app.relay.send(id, &croncave_proto::RelayToAgent::Sleep).await;
    for _ in 0..30 {
        if !app.relay.is_connected(id).await {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    app.relay.disconnect(id).await;
    stop_quietly(app, &c).await;
    let now = app.now();
    let mut tx = app.db.begin().await?;
    sqlx::query("update computers set state = 'asleep', state_changed_at = $2, connected = false, wake_requested_at = null, wake_cause = null where id = $1")
        .bind(id)
        .bind(now)
        .execute(&mut *tx)
        .await?;
    sqlx::query("delete from open_ports where computer_id = $1").bind(id).execute(&mut *tx).await?;
    sqlx::query("update runs set status = 'stopped', status_note = 'Stopped when the computer went to sleep', ended_at = $2 where computer_id = $1 and kind = 'dev_server' and status in ('queued', 'waiting', 'starting', 'running')")
        .bind(id)
        .bind(now)
        .execute(&mut *tx)
        .await?;
    let awake_secs = (now - c.state_changed_at).num_seconds();
    events::emit(
        &mut *tx,
        NewEvent::new(c.account_id, "platform", "computer.slept", "system", format!("{} went to sleep", c.name))
            .computer(id)
            .data(json!({ "cause": cause, "awake_secs": awake_secs })),
    )
    .await?;
    events::live(&mut *tx, c.account_id, "computer", &id.to_string(), json!({ "state": "asleep" })).await?;
    tx.commit().await?;
    Ok(())
}

async fn stop_quietly(app: &AppState, c: &Computer) {
    if let Err(e) = app.providers.compute.stop(c.compute_ref.as_deref().unwrap_or_default()).await {
        tracing::warn!(computer = %c.id, error = %e, "driver stop failed");
    }
}

pub async fn fail_lost_runs(app: &AppState, computer: Uuid, why: &str) -> anyhow::Result<()> {
    let lost: Vec<(Uuid, Uuid)> = sqlx::query_as(
        "update runs set status = 'failed', status_note = $2, error_plain = $2, ended_at = $3
         where computer_id = $1 and status in ('starting', 'running') returning id, account_id",
    )
    .bind(computer)
    .bind(why)
    .bind(app.now())
    .fetch_all(&app.db)
    .await?;
    for (run, account) in lost {
        events::live(&app.db, account, "run", &run.to_string(), json!({})).await?;
    }
    Ok(())
}

/// Record how long a wake took (trigger to ready), for the wake-time target.
pub async fn record_wake(
    app: &AppState,
    computer: Uuid,
    requested_at: chrono::DateTime<Utc>,
    cause: &str,
) -> anyhow::Result<()> {
    let now = app.now();
    let ms = (now - requested_at).num_milliseconds().clamp(0, i32::MAX as i64) as i32;
    sqlx::query(
        "insert into wake_measurements (computer_id, account_id, size, cause, requested_at, ready_at, ms)
         select id, account_id, size, $2, $3, $4, $5 from computers where id = $1",
    )
    .bind(computer)
    .bind(cause)
    .bind(requested_at)
    .bind(now)
    .bind(ms)
    .execute(&app.db)
    .await?;
    Ok(())
}
