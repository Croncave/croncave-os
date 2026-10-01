//! The platform SDK's jobs and runs: every app's work becomes a run with the same record,
//! scheduled by our own Postgres scheduler and dispatched to the computer's agent.

use std::str::FromStr;
use std::time::Duration;

use axum::Json;
use axum::extract::{Path, Query, State};
use chrono::{DateTime, Utc};
use croncave_proto::{RelayToAgent, RunSpec};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::PgConnection;
use uuid::Uuid;

use crate::auth::Auth;
use crate::billing::plan_of;
use crate::error::{ApiError, ApiResult};
use crate::events::{self, NewEvent};
use crate::model::{Computer, Job, Run};
use crate::state::AppState;

// ---------------------------------------------------------------------------------------
// Schedules
// ---------------------------------------------------------------------------------------

/// Accept standard 5-field cron (minute first) or 6-field with seconds.
pub fn parse_schedule(s: &str) -> Result<(String, cron::Schedule), String> {
    let s = s.trim();
    let fields = s.split_whitespace().count();
    let six = match fields {
        5 => format!("0 {s}"),
        6 => s.to_string(),
        _ => {
            return Err(
                "That schedule isn't valid. Pick one of the choices or write a cron line like \"*/15 * * * *\".".into(),
            );
        }
    };
    cron::Schedule::from_str(&six).map(|sch| (six, sch)).map_err(|_| "That schedule isn't valid.".into())
}

/// The shortest gap between runs over the next few runs.
pub fn min_interval_secs(schedule: &cron::Schedule, from: DateTime<Utc>) -> i64 {
    let times: Vec<DateTime<Utc>> = schedule.after(&from).take(25).collect();
    times.windows(2).map(|w| (w[1] - w[0]).num_seconds()).min().unwrap_or(i64::MAX)
}

/// The next run after `after`, reading the schedule's hours in the job's time zone.
pub fn next_after(schedule: &str, zone: &str, after: DateTime<Utc>) -> Option<DateTime<Utc>> {
    let tz = crate::zones::tz(zone);
    parse_schedule(schedule).ok()?.1.after(&after.with_timezone(&tz)).next().map(|t| t.with_timezone(&Utc))
}

/// A schedule in plain words; clock times are in `zone` ("Every day at 9:00 am ET").
pub fn describe_schedule(six: &str, zone: &str) -> String {
    let zone = crate::zones::short(zone);
    let clock_words = |h: u32, m: u32| format!("{} {zone}", clock_words(h, m));
    let f: Vec<&str> = six.split_whitespace().collect();
    if f.len() != 6 || f[0] != "0" {
        return format!("On a custom schedule ({six})");
    }
    let (min, hour, dom, mon, dow) = (f[1], f[2], f[3], f[4], f[5]);
    let any = |x: &str| x == "*" || x == "?";
    if any(dom) && any(mon) && any(dow) {
        if min == "*" && any(hour) {
            return "Every minute".into();
        }
        if let Some(n) = min.strip_prefix("*/").or_else(|| min.strip_prefix("0/"))
            && any(hour)
        {
            return format!("Every {n} minutes");
        }
        if min.parse::<u32>().is_ok() && any(hour) {
            return if min == "0" { "Every hour".into() } else { format!("Every hour at :{min:0>2}") };
        }
        if let (Some(n), Ok(m)) = (hour.strip_prefix("*/"), min.parse::<u32>()) {
            return if m == 0 { format!("Every {n} hours") } else { format!("Every {n} hours at :{m:02}") };
        }
        if let (Ok(h), Ok(m)) = (hour.parse::<u32>(), min.parse::<u32>()) {
            return format!("Every day at {}", clock_words(h, m));
        }
    }
    if any(dom)
        && any(mon)
        && !any(dow)
        && let (Ok(h), Ok(m)) = (hour.parse::<u32>(), min.parse::<u32>())
    {
        let days = match dow {
            "1-5" | "MON-FRI" => "Weekdays".to_string(),
            "0,6" | "6,0" | "SAT,SUN" => "Weekends".to_string(),
            d => format!("On days {d}"),
        };
        return format!("{days} at {}", clock_words(h, m));
    }
    format!("On a custom schedule ({six})")
}

fn clock_words(h: u32, m: u32) -> String {
    let (h12, ampm) = match h {
        0 => (12, "am"),
        1..=11 => (h, "am"),
        12 => (12, "pm"),
        _ => (h - 12, "pm"),
    };
    format!("{h12}:{m:02} {ampm}")
}

pub fn human_duration(secs: i64) -> String {
    match secs {
        s if s < 60 => format!("{s} seconds"),
        s if s < 3600 => format!("{} minute{}", s / 60, if s / 60 == 1 { "" } else { "s" }),
        s => format!("{} hour{}", s / 3600, if s / 3600 == 1 { "" } else { "s" }),
    }
}

// ---------------------------------------------------------------------------------------
// Creating and changing jobs (shared by every app)
// ---------------------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Default)]
pub struct JobOptions {
    pub name: Option<String>,
    /// manual, schedule, files
    pub trigger: Option<String>,
    pub schedule: Option<String>,
    pub watch_path: Option<String>,
    /// skip or queue
    pub overlap: Option<String>,
    pub max_runtime_secs: Option<i32>,
    pub retries: Option<i32>,
    pub max_spend_dollars: Option<f64>,
    pub notify: Option<Value>,
    /// active, paused or draft
    pub status: Option<String>,
}

pub struct ValidOptions {
    pub trigger: String,
    pub schedule: Option<String>,
    pub watch_path: Option<String>,
    pub overlap: String,
    pub max_runtime_secs: i32,
    pub retries: i32,
    pub max_spend_micros: Option<i64>,
}

pub async fn validate_options(
    app: &AppState,
    auth: &Auth,
    o: &JobOptions,
    current: Option<&Job>,
) -> ApiResult<ValidOptions> {
    let (_, plan) = plan_of(app, &auth.account).await?;
    let trigger = o.trigger.clone().or(current.map(|j| j.trigger.clone())).unwrap_or_else(|| "manual".into());
    if !["manual", "schedule", "files"].contains(&trigger.as_str()) {
        return Err(ApiError::bad("Choose by hand, on a schedule, or when files change."));
    }
    let schedule = if trigger == "schedule" {
        let raw = o
            .schedule
            .clone()
            .or(current.and_then(|j| j.schedule.clone()))
            .ok_or_else(|| ApiError::bad("Choose when it runs."))?;
        let (six, sch) = parse_schedule(&raw).map_err(ApiError::bad)?;
        let gap = min_interval_secs(&sch, app.now());
        if gap < plan.min_schedule_secs {
            return Err(ApiError::limit(format!(
                "{} runs jobs at most every {}. Choose a slower schedule or a bigger plan.",
                plan.name,
                human_duration(plan.min_schedule_secs).trim_start_matches("1 ")
            )));
        }
        Some(six)
    } else {
        None
    };
    let watch_path = if trigger == "files" {
        let p = o.watch_path.clone().or(current.and_then(|j| j.watch_path.clone())).unwrap_or_default();
        let p = p.trim().trim_matches('/').to_string();
        if p.contains("..") {
            return Err(ApiError::bad("Choose a folder inside your files."));
        }
        Some(p)
    } else {
        None
    };
    let overlap = o.overlap.clone().or(current.map(|j| j.overlap.clone())).unwrap_or_else(|| "skip".into());
    if !["skip", "queue"].contains(&overlap.as_str()) {
        return Err(ApiError::bad("Choose whether to skip or queue a run while the last one is going."));
    }
    let max_runtime_secs = o.max_runtime_secs.or(current.map(|j| j.max_runtime_secs)).unwrap_or(600);
    if !(5..=86_400).contains(&max_runtime_secs) {
        return Err(ApiError::bad("The longest run time must be between 5 seconds and a day."));
    }
    let retries = o.retries.or(current.map(|j| j.retries)).unwrap_or(0);
    if !(0..=5).contains(&retries) {
        return Err(ApiError::bad("Retries must be between 0 and 5."));
    }
    let max_spend_micros = match o.max_spend_dollars {
        Some(d) if d < 0.0 => return Err(ApiError::bad("The spending limit can't be negative.")),
        Some(d) => Some(crate::catalog::micros(d)),
        None => current.and_then(|j| j.max_spend_micros),
    };
    Ok(ValidOptions { trigger, schedule, watch_path, overlap, max_runtime_secs, retries, max_spend_micros })
}

#[allow(clippy::too_many_arguments)]
pub async fn create_job(
    app: &AppState,
    auth: &Auth,
    computer: &Computer,
    app_id: &str,
    kind: &str,
    name: &str,
    setup: Value,
    opts: &JobOptions,
) -> ApiResult<Job> {
    let v = validate_options(app, auth, opts, None).await?;
    let status = opts.status.clone().unwrap_or_else(|| "active".into());
    if !["active", "paused", "draft"].contains(&status.as_str()) {
        return Err(ApiError::bad("Unknown job status."));
    }
    let name = name.trim();
    if name.is_empty() || name.len() > 80 {
        return Err(ApiError::bad("Give it a name (up to 80 characters)."));
    }
    let next_due = if status == "active" {
        v.schedule.as_deref().and_then(|s| next_after(s, &computer.time_zone, app.now()))
    } else {
        None
    };
    let id = Uuid::new_v4();
    let mut tx = app.db.begin().await?;
    sqlx::query(
        "insert into jobs (id, account_id, computer_id, app, kind, name, setup, trigger, schedule, watch_path, overlap,
                           max_runtime_secs, retries, max_spend_micros, notify, status, next_due_at, created_by_kind, created_by, time_zone)
         values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20)",
    )
    .bind(id)
    .bind(auth.account.id)
    .bind(computer.id)
    .bind(app_id)
    .bind(kind)
    .bind(name)
    .bind(&setup)
    .bind(&v.trigger)
    .bind(&v.schedule)
    .bind(&v.watch_path)
    .bind(&v.overlap)
    .bind(v.max_runtime_secs)
    .bind(v.retries)
    .bind(v.max_spend_micros)
    .bind(opts.notify.clone().unwrap_or_else(|| json!({ "finished": false, "found": true, "failed": true })))
    .bind(&status)
    .bind(next_due)
    .bind(auth.actor_kind)
    .bind(auth.user.id)
    .bind(&computer.time_zone)
    .execute(&mut *tx)
    .await?;
    crate::measure::audit(
        &mut *tx,
        auth.account.id,
        auth.actor_kind,
        Some(auth.user.id),
        "job.created",
        name,
        json!({ "app": app_id, "kind": kind }),
    )
    .await?;
    if status != "draft" {
        events::emit(
            &mut *tx,
            NewEvent::new(auth.account.id, app_id, "job.created", "info", format!("Set up \"{name}\""))
                .computer(computer.id)
                .actor(auth.actor_kind, Some(auth.user.id))
                .data(json!({ "job_id": id })),
        )
        .await?;
    }
    tx.commit().await?;
    if status != "draft" {
        crate::measure::funnel(
            &app.db,
            Some(auth.account.id),
            "job_created",
            json!({ "app": app_id, "kind": kind, "by": auth.actor_kind }),
        )
        .await;
    }
    Ok(sqlx::query_as("select * from jobs where id = $1").bind(id).fetch_one(&app.db).await?)
}

pub async fn load_job(app: &AppState, auth: &Auth, id: Uuid) -> ApiResult<Job> {
    sqlx::query_as("select * from jobs where id = $1 and account_id = $2 and status <> 'deleted'")
        .bind(id)
        .bind(auth.account.id)
        .fetch_optional(&app.db)
        .await?
        .ok_or_else(|| ApiError::not_found("That job"))
}

pub async fn load_run(app: &AppState, auth: &Auth, id: Uuid) -> ApiResult<Run> {
    sqlx::query_as("select * from runs where id = $1 and account_id = $2")
        .bind(id)
        .bind(auth.account.id)
        .fetch_optional(&app.db)
        .await?
        .ok_or_else(|| ApiError::not_found("That run"))
}

/// Queue a run. Scheduled runs are keyed by job and slot, so a slot runs once.
#[allow(clippy::too_many_arguments)]
pub async fn queue_run(
    conn: &mut PgConnection,
    job: &Job,
    trigger: &str,
    started_by: (&str, Option<Uuid>),
    slot_at: Option<DateTime<Utc>>,
    parent: Option<Uuid>,
    attempt: i32,
    setup: Option<Value>,
    now: DateTime<Utc>,
) -> sqlx::Result<Option<Uuid>> {
    let id = Uuid::new_v4();
    let r = sqlx::query(
        "insert into runs (id, job_id, account_id, computer_id, app, kind, trigger, slot_at, started_by_kind, started_by,
                           parent_run_id, attempt, status, setup, queued_at)
         values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, 'queued', $13, $14)
         on conflict (job_id, slot_at) do nothing",
    )
    .bind(id)
    .bind(job.id)
    .bind(job.account_id)
    .bind(job.computer_id)
    .bind(&job.app)
    .bind(&job.kind)
    .bind(trigger)
    .bind(slot_at)
    .bind(started_by.0)
    .bind(started_by.1)
    .bind(parent)
    .bind(attempt)
    .bind(setup.unwrap_or_else(|| job.setup.clone()))
    .bind(now)
    .execute(&mut *conn)
    .await?;
    if r.rows_affected() == 0 {
        return Ok(None);
    }
    events::live(&mut *conn, job.account_id, "run", &id.to_string(), json!({ "job_id": job.id, "status": "queued" }))
        .await?;
    Ok(Some(id))
}

// ---------------------------------------------------------------------------------------
// Scheduler and dispatcher
// ---------------------------------------------------------------------------------------

/// Claim due jobs with row locks and queue one run per slot. Missed slots run once.
pub async fn schedule_due(app: &AppState) -> anyhow::Result<usize> {
    let now = app.now();
    let mut tx = app.db.begin().await?;
    let due: Vec<Job> = sqlx::query_as(
        "select * from jobs where status = 'active' and schedule is not null and next_due_at <= $1
         order by next_due_at for update skip locked limit 100",
    )
    .bind(now)
    .fetch_all(&mut *tx)
    .await?;
    let mut queued = 0;
    for job in &due {
        if queue_run(&mut tx, job, "schedule", ("schedule", None), job.next_due_at, None, 1, None, now).await?.is_some()
        {
            queued += 1;
        }
        let next = job.schedule.as_deref().and_then(|s| next_after(s, &job.time_zone, now));
        sqlx::query("update jobs set next_due_at = $2 where id = $1").bind(job.id).bind(next).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    if queued > 0 {
        app.kick.notify_waiters();
    }
    Ok(queued)
}

pub async fn run_loops(app: AppState) {
    loop {
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(1)) => {}
            _ = app.kick.notified() => {}
        }
        if let Err(e) = schedule_due(&app).await {
            tracing::error!(error = %e, "scheduler failed");
        }
        if let Err(e) = dispatch(&app).await {
            tracing::error!(error = %e, "dispatcher failed");
        }
    }
}

async fn set_status(app: &AppState, run: &Run, status: &str, note: Option<&str>) -> anyhow::Result<()> {
    if run.status == status && run.status_note.as_deref() == note {
        return Ok(());
    }
    let ended = matches!(status, "skipped" | "stopped").then(|| app.now());
    sqlx::query("update runs set status = $2, status_note = $3, ended_at = coalesce($4, ended_at) where id = $1")
        .bind(run.id)
        .bind(status)
        .bind(note)
        .bind(ended)
        .execute(&app.db)
        .await?;
    events::live(&app.db, run.account_id, "run", &run.id.to_string(), json!({ "status": status })).await?;
    Ok(())
}

/// Move queued runs forward: hold at the cap, honour overlap, wake the computer, start.
pub async fn dispatch(app: &AppState) -> anyhow::Result<()> {
    let runs: Vec<Run> = sqlx::query_as(
        "select * from runs where status in ('queued', 'waiting', 'starting') order by queued_at limit 200",
    )
    .fetch_all(&app.db)
    .await?;
    let now = app.now();
    for run in runs {
        if crate::billing::is_paused(&app.db, run.account_id).await? {
            if run.status != "starting" {
                set_status(app, &run, "held", Some("Paused at your spending cap")).await?;
            }
            continue;
        }
        let job: Job = sqlx::query_as("select * from jobs where id = $1").bind(run.job_id).fetch_one(&app.db).await?;
        let computer: Computer =
            sqlx::query_as("select * from computers where id = $1").bind(run.computer_id).fetch_one(&app.db).await?;
        if run.status == "starting" {
            if run.dispatched_at.is_some_and(|d| (now - d).num_seconds() > 20)
                && app.relay.is_connected(computer.id).await
            {
                send_start(app, &run, &job).await?; // The agent ignores a duplicate.
            }
            continue;
        }
        let (others,): (i64,) = sqlx::query_as(
            "select count(*) from runs where job_id = $1 and id <> $2 and status in ('starting', 'running')",
        )
        .bind(job.id)
        .bind(run.id)
        .fetch_one(&app.db)
        .await?;
        if others > 0 {
            if job.overlap == "skip" && matches!(run.trigger.as_str(), "schedule" | "files") {
                set_status(app, &run, "skipped", Some("Skipped: the last run was still going")).await?;
            } else {
                set_status(app, &run, "waiting", Some("Waiting for the last run to finish")).await?;
            }
            continue;
        }
        if run.kind == "agent_task" {
            let account: crate::model::Account =
                sqlx::query_as("select * from accounts where id = $1").bind(run.account_id).fetch_one(&app.db).await?;
            let (_, plan) = plan_of(app, &account).await.map_err(|e| anyhow::anyhow!(e.message))?;
            let (agents,): (i64,) = sqlx::query_as("select count(*) from runs where account_id = $1 and kind = 'agent_task' and status in ('starting', 'running')")
                .bind(run.account_id)
                .fetch_one(&app.db)
                .await?;
            if agents >= plan.parallel_agents {
                set_status(
                    app,
                    &run,
                    "waiting",
                    Some("Waiting for another agent task to finish (your plan runs one at a time)"),
                )
                .await?;
                continue;
            }
        }
        if computer.state == "awake" && app.relay.is_connected(computer.id).await {
            send_start(app, &run, &job).await?;
            continue;
        }
        if !computer.wake_for_schedule && matches!(run.trigger.as_str(), "schedule" | "files") {
            set_status(
                app,
                &run,
                "waiting",
                Some("Waiting until the computer is awake (it doesn't wake for scheduled work)"),
            )
            .await?;
            continue;
        }
        match crate::computers::request_wake(app, &computer, &format!("run:{}", job.name)).await {
            Ok(()) => set_status(app, &run, "waiting", Some("Waking your computer")).await?,
            Err(e) => set_status(app, &run, "waiting", Some(&e.message)).await?,
        }
    }
    Ok(())
}

async fn send_start(app: &AppState, run: &Run, job: &Job) -> anyhow::Result<()> {
    let secrets: Vec<(String, String)> =
        sqlx::query_as::<_, (String, String)>("select name, value_enc from secrets where job_id = $1")
            .bind(job.id)
            .fetch_all(&app.db)
            .await?
            .into_iter()
            .filter_map(|(n, v)| crate::crypto::decrypt(&app.cfg.secrets_key, job.account_id, &v).map(|v| (n, v)))
            .collect();
    let spec = RunSpec {
        run_id: run.id,
        job_id: job.id,
        job_name: job.name.clone(),
        app: job.app.clone(),
        kind: job.kind.clone(),
        setup: sqlx::query_scalar::<_, Value>("select setup from runs where id = $1")
            .bind(run.id)
            .fetch_one(&app.db)
            .await?,
        secrets,
        max_runtime_secs: job.max_runtime_secs as u64,
        attempt: run.attempt as u32,
    };
    if app.relay.send(job.computer_id, &RelayToAgent::StartRun(spec)).await.is_ok() {
        sqlx::query("update runs set status = 'starting', status_note = null, dispatched_at = $2 where id = $1 and status in ('queued', 'waiting', 'starting')")
            .bind(run.id)
            .bind(app.now())
            .execute(&app.db)
            .await?;
        events::live(&app.db, run.account_id, "run", &run.id.to_string(), json!({ "status": "starting" })).await?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------
// API
// ---------------------------------------------------------------------------------------

pub async fn job_view(app: &AppState, job: &Job) -> ApiResult<Value> {
    let last: Option<Run> =
        sqlx::query_as("select * from runs where job_id = $1 and trigger <> 'test' order by queued_at desc limit 1")
            .bind(job.id)
            .fetch_optional(&app.db)
            .await?;
    let mut v = serde_json::to_value(job).unwrap_or_default();
    v["schedule_words"] = json!(job.schedule.as_deref().map(|s| describe_schedule(s, &job.time_zone)));
    v["last_run"] = json!(last.map(|r| run_brief(&r)));
    let secrets: Vec<(String,)> = sqlx::query_as("select name from secrets where job_id = $1 order by name")
        .bind(job.id)
        .fetch_all(&app.db)
        .await?;
    v["secret_names"] = json!(secrets.into_iter().map(|(n,)| n).collect::<Vec<_>>());
    Ok(v)
}

pub fn run_brief(r: &Run) -> Value {
    json!({
        "id": r.id, "status": r.status, "status_note": r.status_note, "trigger": r.trigger, "headline": r.headline,
        "queued_at": r.queued_at, "started_at": r.started_at, "ended_at": r.ended_at, "error_plain": r.error_plain,
        "summary": r.summary, "attempt": r.attempt, "started_by": r.started_by_kind, "data": r.data,
    })
}

#[derive(Deserialize)]
pub struct JobsQuery {
    pub app: Option<String>,
}

pub async fn list(
    State(app): State<AppState>,
    auth: Auth,
    Path(computer): Path<Uuid>,
    Query(q): Query<JobsQuery>,
) -> ApiResult<Json<Value>> {
    crate::computers::load(&app, &auth, computer).await?;
    let jobs: Vec<Job> = sqlx::query_as(
        "select * from jobs where computer_id = $1 and status in ('active', 'paused') and ($2::text is null or app = $2) and kind <> 'dev_server' order by created_at desc",
    )
    .bind(computer)
    .bind(&q.app)
    .fetch_all(&app.db)
    .await?;
    let mut out = Vec::new();
    for j in &jobs {
        out.push(job_view(&app, j).await?);
    }
    Ok(Json(json!({ "jobs": out })))
}

pub async fn get(State(app): State<AppState>, auth: Auth, Path(id): Path<Uuid>) -> ApiResult<Json<Value>> {
    let job = load_job(&app, &auth, id).await?;
    let runs: Vec<Run> = sqlx::query_as("select * from runs where job_id = $1 order by queued_at desc limit 100")
        .bind(id)
        .fetch_all(&app.db)
        .await?;
    let mut v = job_view(&app, &job).await?;
    v["runs"] = json!(runs.iter().map(run_brief).collect::<Vec<_>>());
    if job.app == "watcher" {
        v["rule"] = json!(crate::watcher::plain_rule(&job));
    }
    Ok(Json(v))
}

#[derive(Deserialize)]
pub struct UpdateJob {
    #[serde(flatten)]
    pub opts: JobOptions,
    pub setup: Option<Value>,
}

pub async fn update(
    State(app): State<AppState>,
    auth: Auth,
    Path(id): Path<Uuid>,
    Json(b): Json<UpdateJob>,
) -> ApiResult<Json<Value>> {
    let job = load_job(&app, &auth, id).await?;
    let v = validate_options(&app, &auth, &b.opts, Some(&job)).await?;
    let status = b.opts.status.clone().unwrap_or(job.status.clone());
    if !["active", "paused", "draft"].contains(&status.as_str()) {
        return Err(ApiError::bad("Unknown job status."));
    }
    let setup = match b.setup {
        Some(s) => crate::apps::validate_setup(&app, &auth, &job.app, &job.kind, s).await?,
        None => job.setup.clone(),
    };
    let name = b.opts.name.clone().unwrap_or(job.name.clone());
    let next_due = if status == "active" {
        v.schedule.as_deref().and_then(|s| next_after(s, &job.time_zone, app.now()))
    } else {
        None
    };
    let mut tx = app.db.begin().await?;
    sqlx::query(
        "update jobs set name = $2, setup = $3, trigger = $4, schedule = $5, watch_path = $6, overlap = $7, max_runtime_secs = $8,
                retries = $9, max_spend_micros = $10, notify = coalesce($11, notify), status = $12, next_due_at = $13, updated_at = $14
         where id = $1",
    )
    .bind(id)
    .bind(name.trim())
    .bind(&setup)
    .bind(&v.trigger)
    .bind(&v.schedule)
    .bind(&v.watch_path)
    .bind(&v.overlap)
    .bind(v.max_runtime_secs)
    .bind(v.retries)
    .bind(v.max_spend_micros)
    .bind(&b.opts.notify)
    .bind(&status)
    .bind(next_due)
    .bind(app.now())
    .execute(&mut *tx)
    .await?;
    let action = match (job.status.as_str(), status.as_str()) {
        ("draft", "active") => "job.saved",
        (_, "paused") if job.status != "paused" => "job.paused",
        ("paused", "active") => "job.resumed",
        _ => "job.changed",
    };
    crate::measure::audit(&mut *tx, auth.account.id, auth.actor_kind, Some(auth.user.id), action, &name, json!({}))
        .await?;
    if job.status == "draft" && status == "active" {
        events::emit(
            &mut *tx,
            NewEvent::new(auth.account.id, &job.app, "job.created", "info", format!("Set up \"{name}\""))
                .computer(job.computer_id)
                .actor(auth.actor_kind, Some(auth.user.id)),
        )
        .await?;
    }
    events::live(&mut *tx, auth.account.id, "job", &id.to_string(), json!({})).await?;
    tx.commit().await?;
    if job.status == "draft" && status == "active" {
        crate::measure::funnel(
            &app.db,
            Some(auth.account.id),
            "job_created",
            json!({ "app": job.app, "kind": job.kind, "by": auth.actor_kind }),
        )
        .await;
    }
    let job = load_job(&app, &auth, id).await?;
    Ok(Json(job_view(&app, &job).await?))
}

pub async fn delete(State(app): State<AppState>, auth: Auth, Path(id): Path<Uuid>) -> ApiResult<Json<Value>> {
    let job = load_job(&app, &auth, id).await?;
    let now = app.now();
    sqlx::query("update jobs set status = 'deleted', next_due_at = null, updated_at = $2 where id = $1")
        .bind(id)
        .bind(now)
        .execute(&app.db)
        .await?;
    sqlx::query("update runs set status = 'stopped', status_note = 'The job was deleted', ended_at = $2 where job_id = $1 and status in ('queued', 'waiting', 'held')")
        .bind(id)
        .bind(now)
        .execute(&app.db)
        .await?;
    let running: Vec<(Uuid,)> =
        sqlx::query_as("select id from runs where job_id = $1 and status in ('starting', 'running')")
            .bind(id)
            .fetch_all(&app.db)
            .await?;
    for (r,) in running {
        let _ = app.relay.send(job.computer_id, &RelayToAgent::StopRun { run_id: r }).await;
    }
    crate::measure::audit(
        &app.db,
        auth.account.id,
        auth.actor_kind,
        Some(auth.user.id),
        "job.deleted",
        &job.name,
        json!({}),
    )
    .await?;
    events::live(&app.db, auth.account.id, "job", &id.to_string(), json!({})).await?;
    Ok(Json(json!({ "ok": true })))
}

/// Run now / Check now.
pub async fn run_now(State(app): State<AppState>, auth: Auth, Path(id): Path<Uuid>) -> ApiResult<Json<Value>> {
    let job = load_job(&app, &auth, id).await?;
    let run = start_now(&app, &auth, &job, "manual", None).await?;
    Ok(Json(json!({ "run_id": run })))
}

pub async fn start_now(app: &AppState, auth: &Auth, job: &Job, trigger: &str, setup: Option<Value>) -> ApiResult<Uuid> {
    if crate::billing::is_paused(&app.db, auth.account.id).await? {
        return Err(ApiError::limit(
            "Work is paused at your spending cap. Raise the cap in Plans and billing to run this.",
        )
        .with_code("paused"));
    }
    let mut conn = app.db.acquire().await?;
    let run =
        queue_run(&mut conn, job, trigger, (auth.actor_kind, Some(auth.user.id)), None, None, 1, setup, app.now())
            .await?
            .ok_or_else(|| ApiError::conflict("That run is already queued."))?;
    app.kick.notify_waiters();
    Ok(run)
}

pub async fn get_run(State(app): State<AppState>, auth: Auth, Path(id): Path<Uuid>) -> ApiResult<Json<Value>> {
    let run = load_run(&app, &auth, id).await?;
    let job: Job = sqlx::query_as("select * from jobs where id = $1").bind(run.job_id).fetch_one(&app.db).await?;
    let output: Vec<(i64, String, String, DateTime<Utc>)> = sqlx::query_as(
        "select id, stream, text, at from (select * from run_output where run_id = $1 order by id desc limit 2000) o order by id",
    )
    .bind(id)
    .fetch_all(&app.db)
    .await?;
    let approvals: Vec<(String, String, String, String, DateTime<Utc>)> = sqlx::query_as(
        "select request_id, command, reason, status, created_at from approvals where run_id = $1 order by created_at",
    )
    .bind(id)
    .fetch_all(&app.db)
    .await?;
    let mut v = serde_json::to_value(&run).unwrap_or_default();
    v["job"] = json!({ "id": job.id, "name": job.name, "app": job.app, "kind": job.kind, "max_runtime_secs": job.max_runtime_secs });
    v["output"] = json!(
        output
            .into_iter()
            .map(|(i, s, t, at)| json!({ "id": i, "stream": s, "text": t, "at": at }))
            .collect::<Vec<_>>()
    );
    v["approvals"] = json!(
        approvals
            .into_iter()
            .map(|(r, c, why, s, at)| json!({ "request_id": r, "command": c, "reason": why, "status": s, "at": at }))
            .collect::<Vec<_>>()
    );
    Ok(Json(v))
}

#[derive(Deserialize)]
pub struct OutputQuery {
    pub after: Option<i64>,
}

pub async fn run_output(
    State(app): State<AppState>,
    auth: Auth,
    Path(id): Path<Uuid>,
    Query(q): Query<OutputQuery>,
) -> ApiResult<Json<Value>> {
    load_run(&app, &auth, id).await?;
    let output: Vec<(i64, String, String, DateTime<Utc>)> = sqlx::query_as(
        "select id, stream, text, at from run_output where run_id = $1 and id > $2 order by id limit 5000",
    )
    .bind(id)
    .bind(q.after.unwrap_or(0))
    .fetch_all(&app.db)
    .await?;
    Ok(Json(
        json!({ "output": output.into_iter().map(|(i, s, t, at)| json!({ "id": i, "stream": s, "text": t, "at": at })).collect::<Vec<_>>() }),
    ))
}

pub async fn stop_run(State(app): State<AppState>, auth: Auth, Path(id): Path<Uuid>) -> ApiResult<Json<Value>> {
    let run = load_run(&app, &auth, id).await?;
    match run.status.as_str() {
        "queued" | "waiting" | "held" => {
            sqlx::query("update runs set status = 'stopped', status_note = 'Stopped before it started', ended_at = $2 where id = $1").bind(id).bind(app.now()).execute(&app.db).await?;
            events::live(&app.db, run.account_id, "run", &id.to_string(), json!({ "status": "stopped" })).await?;
        }
        "starting" | "running" => {
            app.relay.send(run.computer_id, &RelayToAgent::StopRun { run_id: id }).await?;
            sqlx::query("update runs set status_note = 'Stopping…' where id = $1").bind(id).execute(&app.db).await?;
        }
        _ => return Err(ApiError::conflict("This run isn't going.")),
    }
    crate::measure::audit(
        &app.db,
        auth.account.id,
        auth.actor_kind,
        Some(auth.user.id),
        "run.stopped",
        &id.to_string(),
        json!({}),
    )
    .await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn retry_run(State(app): State<AppState>, auth: Auth, Path(id): Path<Uuid>) -> ApiResult<Json<Value>> {
    let run = load_run(&app, &auth, id).await?;
    let job = load_job(&app, &auth, run.job_id).await?;
    if crate::model::ACTIVE_RUN_STATES.contains(&run.status.as_str()) {
        return Err(ApiError::conflict("This run is still going."));
    }
    let mut conn = app.db.acquire().await?;
    let new = queue_run(
        &mut conn,
        &job,
        "retry",
        (auth.actor_kind, Some(auth.user.id)),
        None,
        Some(run.id),
        run.attempt + 1,
        None,
        app.now(),
    )
    .await?
    .ok_or_else(|| ApiError::conflict("Already queued."))?;
    app.kick.notify_waiters();
    Ok(Json(json!({ "run_id": new })))
}

#[derive(Deserialize)]
pub struct ApprovalBody {
    pub approved: bool,
}

pub async fn approve(
    State(app): State<AppState>,
    auth: Auth,
    Path((id, request)): Path<(Uuid, String)>,
    Json(b): Json<ApprovalBody>,
) -> ApiResult<Json<Value>> {
    let run = load_run(&app, &auth, id).await?;
    let updated = sqlx::query("update approvals set status = $3, decided_by = $4, decided_at = $5 where run_id = $1 and request_id = $2 and status = 'pending'")
        .bind(id)
        .bind(&request)
        .bind(if b.approved { "approved" } else { "denied" })
        .bind(auth.user.id)
        .bind(app.now())
        .execute(&app.db)
        .await?;
    if updated.rows_affected() == 0 {
        return Err(ApiError::conflict("That was already answered."));
    }
    app.relay
        .send(
            run.computer_id,
            &RelayToAgent::Approval { run_id: id, request_id: request.clone(), approved: b.approved },
        )
        .await?;
    crate::measure::audit(
        &app.db,
        auth.account.id,
        auth.actor_kind,
        Some(auth.user.id),
        if b.approved { "agent.command_approved" } else { "agent.command_denied" },
        &request,
        json!({}),
    )
    .await?;
    events::live(&app.db, run.account_id, "run", &id.to_string(), json!({})).await?;
    events::live(&app.db, run.account_id, "computer", &run.computer_id.to_string(), json!({})).await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct SecretBody {
    pub name: String,
    pub value: String,
}

pub async fn set_secret(
    State(app): State<AppState>,
    auth: Auth,
    Path(id): Path<Uuid>,
    Json(b): Json<SecretBody>,
) -> ApiResult<Json<Value>> {
    load_job(&app, &auth, id).await?;
    let name = b.name.trim().to_uppercase();
    if name.is_empty()
        || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        || name.starts_with(|c: char| c.is_ascii_digit())
    {
        return Err(ApiError::bad("Secret names are letters, digits and underscores, like API_KEY."));
    }
    if b.value.is_empty() {
        return Err(ApiError::bad("The secret needs a value."));
    }
    let enc = crate::crypto::encrypt(&app.cfg.secrets_key, auth.account.id, &b.value);
    sqlx::query("insert into secrets (id, account_id, job_id, name, value_enc) values ($1, $2, $3, $4, $5) on conflict (job_id, name) do update set value_enc = excluded.value_enc")
        .bind(Uuid::new_v4())
        .bind(auth.account.id)
        .bind(id)
        .bind(&name)
        .bind(enc)
        .execute(&app.db)
        .await?;
    crate::measure::audit(
        &app.db,
        auth.account.id,
        auth.actor_kind,
        Some(auth.user.id),
        "secret.set",
        &name,
        json!({ "job_id": id }),
    )
    .await?;
    Ok(Json(json!({ "ok": true, "name": name })))
}

pub async fn delete_secret(
    State(app): State<AppState>,
    auth: Auth,
    Path((id, name)): Path<(Uuid, String)>,
) -> ApiResult<Json<Value>> {
    load_job(&app, &auth, id).await?;
    sqlx::query("delete from secrets where job_id = $1 and name = $2").bind(id).bind(&name).execute(&app.db).await?;
    crate::measure::audit(
        &app.db,
        auth.account.id,
        auth.actor_kind,
        Some(auth.user.id),
        "secret.deleted",
        &name,
        json!({ "job_id": id }),
    )
    .await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn computer_runs(State(app): State<AppState>, auth: Auth, Path(id): Path<Uuid>) -> ApiResult<Json<Value>> {
    crate::computers::load(&app, &auth, id).await?;
    let runs: Vec<(Uuid, String, String, String, String, Option<String>, DateTime<Utc>, Option<DateTime<Utc>>)> = sqlx::query_as(
        "select r.id, j.name, r.app, r.kind, r.status, r.headline, r.queued_at, r.ended_at from runs r join jobs j on j.id = r.job_id
         where r.computer_id = $1 and r.trigger <> 'test' order by r.queued_at desc limit 30",
    )
    .bind(id)
    .fetch_all(&app.db)
    .await?;
    Ok(Json(
        json!({ "runs": runs.into_iter().map(|(id, name, a, k, s, h, q, e)| json!({ "id": id, "job_name": name, "app": a, "kind": k, "status": s, "headline": h, "queued_at": q, "ended_at": e })).collect::<Vec<_>>() }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schedules_parse_and_read_plainly() {
        let (six, _) = parse_schedule("*/15 * * * *").unwrap();
        assert_eq!(six, "0 */15 * * * *");
        assert_eq!(describe_schedule(&six, "America/New_York"), "Every 15 minutes");
        assert_eq!(describe_schedule("0 0 * * * *", "America/New_York"), "Every hour");
        assert_eq!(describe_schedule("0 30 9 * * *", "America/New_York"), "Every day at 9:30 am ET");
        assert_eq!(describe_schedule("0 0 18 * * 1-5", "America/Los_Angeles"), "Weekdays at 6:00 pm PT");
    }

    #[test]
    fn schedules_run_at_the_hour_in_the_jobs_time_zone() {
        use chrono::TimeZone;
        // 9 AM every day, asked at noon UTC on a summer day.
        let at = Utc.with_ymd_and_hms(2026, 7, 1, 12, 0, 0).unwrap();
        // 9 AM in New York (UTC-4) is 13:00 UTC, still to come today.
        assert_eq!(
            next_after("0 9 * * *", "America/New_York", at),
            Some(Utc.with_ymd_and_hms(2026, 7, 1, 13, 0, 0).unwrap())
        );
        // 9 AM in Los Angeles (UTC-7) is 16:00 UTC.
        assert_eq!(
            next_after("0 9 * * *", "America/Los_Angeles", at),
            Some(Utc.with_ymd_and_hms(2026, 7, 1, 16, 0, 0).unwrap())
        );
        // In winter New York is UTC-5, so 9 AM is 14:00 UTC.
        let winter = Utc.with_ymd_and_hms(2026, 1, 15, 12, 0, 0).unwrap();
        assert_eq!(
            next_after("0 9 * * *", "America/New_York", winter),
            Some(Utc.with_ymd_and_hms(2026, 1, 15, 14, 0, 0).unwrap())
        );
        assert!(parse_schedule("every day").is_err());
    }

    #[test]
    fn the_shortest_gap_is_found() {
        let (_, s) = parse_schedule("*/5 * * * *").unwrap();
        assert_eq!(min_interval_secs(&s, Utc::now()), 300);
        let (_, s) = parse_schedule("0 9,10 * * *").unwrap();
        assert_eq!(min_interval_secs(&s, Utc::now()), 3600);
    }
}
