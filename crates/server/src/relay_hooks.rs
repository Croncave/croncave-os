//! What the relay needs from the control plane: bootstrap tokens, connection state, and
//! storing what agents report (exactly once, even when an agent resends after a
//! reconnect).

use std::sync::Weak;

use async_trait::async_trait;
use croncave_proto::{AgentEvent, ChangeKind, Hello, RunOutcome, RunResult, SdkCall, SeqEvent};
use croncave_relay::RelayHooks;
use serde_json::{Value, json};
use sqlx::PgConnection;
use uuid::Uuid;

use crate::events::{self, NewEvent};
use crate::model::{Job, Run};
use crate::state::App;

pub struct Hooks {
    pub app: Weak<App>,
}

impl Hooks {
    fn app(&self) -> Option<std::sync::Arc<App>> {
        self.app.upgrade()
    }
}

#[async_trait]
impl RelayHooks for Hooks {
    async fn redeem_bootstrap(&self, token: &str) -> Option<Uuid> {
        let app = self.app()?;
        sqlx::query_scalar(
            "update computers set bootstrap_hash = null where bootstrap_hash = $1 and bootstrap_expires_at > $2 and deleted_at is null returning id",
        )
        .bind(crate::crypto::hash(token))
        .bind(app.real_now())
        .fetch_optional(&app.db)
        .await
        .ok()
        .flatten()
    }

    async fn authorize(&self, computer: Uuid) -> bool {
        let Some(app) = self.app() else { return false };
        sqlx::query_scalar::<_, bool>("select exists(select 1 from computers where id = $1 and deleted_at is null)")
            .bind(computer)
            .fetch_one(&app.db)
            .await
            .unwrap_or(false)
    }

    async fn connected(&self, computer: Uuid, hello: &Hello) {
        let Some(app) = self.app() else { return };
        let r = sqlx::query(
            "update computers set connected = true, agent_version = $2,
                    agent_acked_seq = case when agent_epoch = $3 then agent_acked_seq else 0 end, agent_epoch = $3
             where id = $1",
        )
        .bind(computer)
        .bind(&hello.agent_version)
        .bind(hello.outbox_epoch)
        .execute(&app.db)
        .await;
        if let Err(e) = r {
            tracing::error!(error = %e, "recording connection failed");
        }
        if let Err(e) = crate::orchestrator::mark_awake(&app, computer).await {
            tracing::error!(error = %e, "marking computer awake failed");
        }
        // Runs that were running before this connection, and that the agent no longer has,
        // were lost (the computer restarted). Runs started after it are not suspects. Give a
        // resent RunFinished a moment to arrive first.
        let suspects: Vec<Uuid> = sqlx::query_scalar(
            "select id from runs where computer_id = $1 and status = 'running' and not (id = any($2))",
        )
        .bind(computer)
        .bind(&hello.running_runs)
        .fetch_all(&app.db)
        .await
        .unwrap_or_default();
        app.kick.notify_waiters();
        if suspects.is_empty() {
            return;
        }
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            let lost: Vec<(Uuid, Uuid)> = sqlx::query_as(
                "update runs set status = 'failed', status_note = 'The computer restarted during this run.', error_plain = 'The computer restarted during this run.',
                        error_fix = 'Retry it.', ended_at = $2
                 where id = any($1) and status = 'running' returning id, account_id",
            )
            .bind(&suspects)
            .bind(app.now())
            .fetch_all(&app.db)
            .await
            .unwrap_or_default();
            for (run, account) in lost {
                let _ = events::live(&app.db, account, "run", &run.to_string(), json!({})).await;
            }
        });
    }

    async fn disconnected(&self, computer: Uuid) {
        let Some(app) = self.app() else { return };
        let _ =
            sqlx::query("update computers set connected = false where id = $1").bind(computer).execute(&app.db).await;
    }

    async fn events(&self, computer: Uuid, events: Vec<SeqEvent>) -> anyhow::Result<()> {
        let app = self.app().ok_or_else(|| anyhow::anyhow!("shutting down"))?;
        let mut tx = app.db.begin().await?;
        let (account, acked): (Uuid, i64) =
            sqlx::query_as("select account_id, agent_acked_seq from computers where id = $1 for update")
                .bind(computer)
                .fetch_one(&mut *tx)
                .await?;
        let mut max = acked;
        let mut finished = Vec::new();
        for ev in events {
            if (ev.seq as i64) <= acked {
                continue; // Already stored: a resend after a reconnect.
            }
            max = max.max(ev.seq as i64);
            if let Some(done) = apply(&app, &mut tx, computer, account, ev).await? {
                finished.push(done);
            }
        }
        sqlx::query("update computers set agent_acked_seq = $2 where id = $1")
            .bind(computer)
            .bind(max)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        for (account, run) in finished {
            crate::notify_worker::kick(&app);
            let _ = (account, run);
        }
        Ok(())
    }

    async fn sdk_call(&self, _computer: Uuid, call: SdkCall) -> Result<Value, String> {
        let app = self.app().ok_or("shutting down")?;
        match call {
            SdkCall::MarketQuote { symbol } => {
                let q = app.providers.market.quote(&symbol, app.now()).await?;
                Ok(serde_json::to_value(q).map_err(|e| e.to_string())?)
            }
        }
    }
}

async fn apply(
    app: &App,
    tx: &mut PgConnection,
    computer: Uuid,
    account: Uuid,
    ev: SeqEvent,
) -> anyhow::Result<Option<(Uuid, Uuid)>> {
    let now = app.now();
    match ev.event {
        AgentEvent::RunStarted { run_id } => {
            sqlx::query("update runs set status = 'running', started_at = coalesce(started_at, $2), status_note = null where id = $1 and status in ('queued', 'waiting', 'starting')")
                .bind(run_id)
                .bind(now)
                .execute(&mut *tx)
                .await?;
            events::live(&mut *tx, account, "run", &run_id.to_string(), json!({ "status": "running" })).await?;
        }
        AgentEvent::RunOutput { run_id, stream, text } => {
            let stream = serde_json::to_value(stream)?.as_str().unwrap_or("stdout").to_string();
            let text: String = text.chars().take(4000).collect();
            let (id,): (i64,) = sqlx::query_as("insert into run_output (run_id, stream, text, at) select $1, $2, $3, $4 where exists (select 1 from runs where id = $1) returning id")
                .bind(run_id)
                .bind(&stream)
                .bind(&text)
                .bind(now)
                .fetch_optional(&mut *tx)
                .await?
                .unwrap_or((0,));
            events::live(
                &mut *tx,
                account,
                "output",
                &run_id.to_string(),
                json!({ "id": id, "stream": stream, "text": text.chars().take(1500).collect::<String>() }),
            )
            .await?;
        }
        AgentEvent::RunProgress { run_id, cpu_percent, memory_mb, message } => {
            sqlx::query("update runs set progress = $2 where id = $1")
                .bind(run_id)
                .bind(json!({ "cpu_percent": cpu_percent, "memory_mb": memory_mb, "message": message }))
                .execute(&mut *tx)
                .await?;
            events::live(
                &mut *tx,
                account,
                "progress",
                &run_id.to_string(),
                json!({ "cpu_percent": cpu_percent, "memory_mb": memory_mb }),
            )
            .await?;
        }
        AgentEvent::NeedsApproval { run_id, request_id, command, reason } => {
            let job: Option<(Uuid, String, String)> =
                sqlx::query_as("select j.id, j.name, j.app from runs r join jobs j on j.id = r.job_id where r.id = $1")
                    .bind(run_id)
                    .fetch_optional(&mut *tx)
                    .await?;
            let Some((job_id, job_name, app_id)) = job else { return Ok(None) };
            sqlx::query("insert into approvals (id, run_id, request_id, command, reason) values ($1, $2, $3, $4, $5) on conflict do nothing")
                .bind(Uuid::new_v4())
                .bind(run_id)
                .bind(&request_id)
                .bind(&command)
                .bind(&reason)
                .execute(&mut *tx)
                .await?;
            events::emit(
                &mut *tx,
                NewEvent::new(
                    account,
                    &app_id,
                    "agent.needs_approval",
                    "needs_you",
                    format!("The agent wants to run \"{command}\""),
                )
                .body(format!("{reason} Approve or deny it in \"{job_name}\"."))
                .computer(computer)
                .actor("agent", None)
                .run(run_id, job_id)
                .urgent(),
            )
            .await?;
            events::live(&mut *tx, account, "computer", &computer.to_string(), json!({})).await?;
        }
        AgentEvent::RunFinished(result) => return finish(app, tx, computer, account, result).await,
        AgentEvent::PortChanged { port, open, run_id } => {
            if open {
                sqlx::query("insert into open_ports (computer_id, port, run_id, opened_at) values ($1, $2, $3, $4) on conflict (computer_id, port) do update set run_id = excluded.run_id")
                    .bind(computer)
                    .bind(port as i32)
                    .bind(run_id)
                    .bind(now)
                    .execute(&mut *tx)
                    .await?;
            } else {
                sqlx::query("delete from open_ports where computer_id = $1 and port = $2")
                    .bind(computer)
                    .bind(port as i32)
                    .execute(&mut *tx)
                    .await?;
            }
            events::live(&mut *tx, account, "computer", &computer.to_string(), json!({ "port": port, "open": open }))
                .await?;
        }
        AgentEvent::Health(h) => {
            sqlx::query("update computers set health = $2 where id = $1")
                .bind(computer)
                .bind(serde_json::to_value(&h)?)
                .execute(&mut *tx)
                .await?;
            events::live(&mut *tx, account, "health", &computer.to_string(), serde_json::to_value(&h)?).await?;
        }
    }
    Ok(None)
}

async fn finish(
    app: &App,
    tx: &mut PgConnection,
    computer: Uuid,
    account: Uuid,
    r: RunResult,
) -> anyhow::Result<Option<(Uuid, Uuid)>> {
    let Some(run): Option<Run> =
        sqlx::query_as("select * from runs where id = $1 for update").bind(r.run_id).fetch_optional(&mut *tx).await?
    else {
        return Ok(None);
    };
    // A run marked lost may still report back; a run already closed may not change.
    let lost = run.status == "failed"
        && run.status_note.as_deref().is_some_and(|n| n.contains("restarted") || n.contains("stopped unexpectedly"));
    if !crate::model::ACTIVE_RUN_STATES.contains(&run.status.as_str()) && !lost && run.kind != "dev_server" {
        return Ok(None);
    }
    let job: Job = sqlx::query_as("select * from jobs where id = $1").bind(run.job_id).fetch_one(&mut *tx).await?;
    let now = app.now();
    let status = match r.outcome {
        RunOutcome::Succeeded => "succeeded",
        RunOutcome::Failed => "failed",
        RunOutcome::Stopped => "stopped",
        RunOutcome::TimedOut => "timed_out",
    };
    let explanation = crate::explain::explain(
        &job.app,
        status,
        r.exit_code,
        &r.output_tail,
        r.error.as_deref(),
        job.max_runtime_secs as i64,
    );
    let changed = r.changes.iter().filter(|c| c.kind != ChangeKind::Deleted).count();
    let deleted = r.changes.len() - changed;
    let headline =
        r.summary.as_ref().map(|s| s.headline.clone()).filter(|h| !h.is_empty()).unwrap_or_else(|| match status {
            "succeeded" if r.changes.is_empty() => "Finished".to_string(),
            "succeeded" => format!(
                "Finished · {} file{} changed{}",
                changed,
                if changed == 1 { "" } else { "s" },
                if deleted > 0 { format!(", {deleted} deleted") } else { String::new() }
            ),
            "stopped" => "Stopped".to_string(),
            "timed_out" => "Stopped at its time limit".to_string(),
            _ => "Failed".to_string(),
        });
    let started = run.started_at.unwrap_or(now);
    sqlx::query(
        "update runs set status = $2, ended_at = $3, exit_code = $4, headline = $5, summary = $6, output_tail = $7, error_plain = $8,
                error_fix = $9, data = $10, changes = $11, awake_seconds = $12, status_note = null
         where id = $1",
    )
    .bind(run.id)
    .bind(status)
    .bind(now)
    .bind(r.exit_code)
    .bind(&headline)
    .bind(serde_json::to_value(&r.summary)?)
    .bind(&r.output_tail)
    .bind(explanation.as_ref().map(|e| e.plain.clone()))
    .bind(explanation.as_ref().map(|e| e.fix.clone()))
    .bind(&r.data)
    .bind(serde_json::to_value(&r.changes)?)
    .bind((now - started).num_seconds().max(0) as i32)
    .execute(&mut *tx)
    .await?;

    // Where files came from.
    let actor = if run.kind == "agent_task" { "agent" } else { "run" };
    for c in &r.changes {
        sqlx::query("insert into file_records (computer_id, path, change, actor_kind, actor_id, run_id, size, at) values ($1, $2, $3, $4, $5, $5, $6, $7)")
            .bind(computer)
            .bind(&c.path)
            .bind(serde_json::to_value(c.kind)?.as_str().unwrap_or("changed"))
            .bind(actor)
            .bind(run.id)
            .bind(c.size as i64)
            .bind(now)
            .execute(&mut *tx)
            .await?;
    }
    let paths: Vec<String> = r.changes.iter().map(|c| c.path.clone()).collect();
    queue_file_triggers(app, tx, computer, Some(job.id), &paths).await?;

    let is_test = run.trigger == "test";
    let notify_on = |k: &str| job.notify.get(k).and_then(Value::as_bool).unwrap_or(k != "finished");
    let base = |level: &'static str, kind: &str, title: String| {
        NewEvent::new(account, &job.app, kind, level, title)
            .computer(computer)
            .run(run.id, job.id)
            .actor(if run.trigger == "schedule" { "schedule" } else { "user" }, None)
    };
    let event = match (job.app.as_str(), status) {
        _ if is_test => Some(base("system", "run.test", format!("Tested \"{}\": {}", job.name, headline))),
        (_, "stopped") if job.kind == "dev_server" => None,
        ("watcher", "succeeded") => {
            let matched = r.data.get("matched").and_then(Value::as_bool).unwrap_or(false);
            if matched {
                let ev = base("success", "watch.found", format!("{}: {}", job.name, headline))
                    .data(r.data.get("detail").cloned().unwrap_or_default());
                Some(if notify_on("found") { ev.notify() } else { ev })
            } else {
                Some(base("system", "watch.checked", format!("{}: {}", job.name, headline)))
            }
        }
        ("code", "succeeded") if job.kind == "agent_task" => Some(
            base("needs_you", "agent.review_ready", format!("Review the agent's changes: {}", job.name))
                .body(headline.clone())
                .actor("agent", None)
                .notify(),
        ),
        (_, "succeeded") => {
            let ev = base("success", "run.finished", format!("{}: {}", job.name, headline));
            Some(if notify_on("finished") { ev.notify() } else { ev })
        }
        (_, "stopped") => Some(base("info", "run.stopped", format!("Stopped \"{}\"", job.name))),
        _ => {
            let title = if job.app == "watcher" {
                format!("Couldn't check \"{}\"", job.name)
            } else {
                format!("\"{}\" failed", job.name)
            };
            let ev = base("failed", "run.failed", title)
                .body(explanation.as_ref().map(|e| format!("{} {}", e.plain, e.fix)).unwrap_or_default());
            Some(if notify_on("failed") { ev.notify() } else { ev })
        }
    };
    if let Some(ev) = event {
        events::emit(&mut *tx, ev).await?;
    }
    if matches!(status, "failed" | "timed_out") && !is_test && run.attempt <= job.retries && job.status == "active" {
        crate::jobs::queue_run(tx, &job, "retry", ("system", None), None, Some(run.id), run.attempt + 1, None, now)
            .await?;
    }
    if !is_test && status == "succeeded" {
        let (first,): (i64,) = sqlx::query_as(
            "select count(*) from runs where account_id = $1 and status = 'succeeded' and trigger <> 'test'",
        )
        .bind(account)
        .fetch_one(&mut *tx)
        .await?;
        if first == 1 {
            sqlx::query("insert into funnel_events (account_id, kind, data) select $1, 'first_result', json_build_object('app', $2::text, 'secs_since_signup', extract(epoch from ($3 - created_at))) from accounts where id = $1")
                .bind(account)
                .bind(&job.app)
                .bind(now)
                .execute(&mut *tx)
                .await?;
        }
    }
    events::live(&mut *tx, account, "run", &run.id.to_string(), json!({ "status": status, "job_id": job.id })).await?;
    events::live(&mut *tx, account, "computer", &computer.to_string(), json!({})).await?;
    app.kick.notify_waiters();
    Ok(Some((account, run.id)))
}

/// Queue runs for jobs that run "when files change" in a folder these paths are in.
pub async fn queue_file_triggers(
    app: &App,
    tx: &mut PgConnection,
    computer: Uuid,
    except_job: Option<Uuid>,
    paths: &[String],
) -> anyhow::Result<()> {
    if paths.is_empty() {
        return Ok(());
    }
    let jobs: Vec<Job> =
        sqlx::query_as("select * from jobs where computer_id = $1 and status = 'active' and trigger = 'files'")
            .bind(computer)
            .fetch_all(&mut *tx)
            .await?;
    for job in jobs {
        if Some(job.id) == except_job {
            continue; // A job never triggers itself.
        }
        let folder = job.watch_path.clone().unwrap_or_default();
        let hit = paths.iter().any(|p| folder.is_empty() || p == &folder || p.starts_with(&format!("{folder}/")));
        if !hit {
            continue;
        }
        let (pending,): (i64,) =
            sqlx::query_as("select count(*) from runs where job_id = $1 and status in ('queued', 'waiting')")
                .bind(job.id)
                .fetch_one(&mut *tx)
                .await?;
        if pending == 0 {
            crate::jobs::queue_run(tx, &job, "files", ("schedule", None), None, None, 1, None, app.now()).await?;
        }
    }
    Ok(())
}
