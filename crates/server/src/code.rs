//! The Code app's control-plane side: projects, agent tasks with review, dev servers and
//! private previews.

use axum::Json;
use axum::extract::{Path, Query, State};
use bytes::Bytes;
use croncave_proto::{CodeReviewRequest, FileDiff, FileEntry, FilesRequest, StreamOpen};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::auth::Auth;
use crate::error::{ApiError, ApiResult};
use crate::jobs::JobOptions;
use crate::model::Job;
use crate::state::AppState;

pub const PROJECTS: &str = "Projects";

fn project_path(name: &str) -> ApiResult<String> {
    let n = name.trim().trim_matches('/');
    if n.is_empty() || n.contains("..") || n.contains('/') {
        return Err(ApiError::bad("Project names can't contain slashes."));
    }
    Ok(format!("{PROJECTS}/{n}"))
}

pub async fn projects(State(app): State<AppState>, auth: Auth, Path(computer): Path<Uuid>) -> ApiResult<Json<Value>> {
    let c = crate::computers::load(&app, &auth, computer).await?;
    crate::computers::ensure_awake(&app, &c, "open:code").await?;
    let res: Result<(Vec<FileEntry>, _), _> =
        app.relay.request(c.id, &StreamOpen::Files(FilesRequest::List { path: PROJECTS.into() }), None).await;
    let list = match res {
        Ok((l, _)) => l.into_iter().filter(|e| e.is_dir).collect(),
        Err(_) => vec![],
    };
    Ok(Json(json!({ "projects": list })))
}

#[derive(Deserialize)]
pub struct NewProject {
    pub name: String,
}

pub async fn create_project(
    State(app): State<AppState>,
    auth: Auth,
    Path(computer): Path<Uuid>,
    Json(b): Json<NewProject>,
) -> ApiResult<Json<Value>> {
    let c = crate::computers::load(&app, &auth, computer).await?;
    let dir = project_path(&b.name)?;
    crate::computers::ensure_awake(&app, &c, "open:code").await?;
    let files = [
        ("index.html", include_str!("../templates/static-site/index.html")),
        ("style.css", include_str!("../templates/static-site/style.css")),
        ("README.md", include_str!("../templates/static-site/README.md")),
    ];
    let mut written = vec![];
    for (name, body) in files {
        let path = format!("{dir}/{name}");
        crate::scripts::write_file(&app, &c, &path, Bytes::from_static(body.as_bytes())).await?;
        written.push(path);
    }
    crate::files_api::record_user_change(&app, &auth, c.id, &written, "created").await?;
    Ok(Json(json!({ "project": dir })))
}

#[derive(Deserialize)]
pub struct TreeQuery {
    pub project: String,
}

/// Every file in a project, for the editor's file tree.
pub async fn tree(
    State(app): State<AppState>,
    auth: Auth,
    Path(computer): Path<Uuid>,
    Query(q): Query<TreeQuery>,
) -> ApiResult<Json<Value>> {
    let c = crate::computers::load(&app, &auth, computer).await?;
    crate::computers::ensure_awake(&app, &c, "open:code").await?;
    let mut out = vec![];
    let mut stack = vec![(q.project.trim_matches('/').to_string(), 0)];
    while let Some((dir, depth)) = stack.pop() {
        let (entries, _): (Vec<FileEntry>, _) =
            app.relay.request(c.id, &StreamOpen::Files(FilesRequest::List { path: dir.clone() }), None).await?;
        for e in entries {
            if e.is_dir && depth < 5 && !["node_modules", ".git", "__pycache__"].contains(&e.name.as_str()) {
                stack.push((e.path.clone(), depth + 1));
            }
            out.push(e);
        }
        if out.len() > 2000 {
            break;
        }
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(Json(json!({ "files": out })))
}

#[derive(Deserialize)]
pub struct NewTask {
    pub project: String,
    pub prompt: String,
}

pub async fn create_task(
    State(app): State<AppState>,
    auth: Auth,
    Path(computer): Path<Uuid>,
    Json(b): Json<NewTask>,
) -> ApiResult<Json<Value>> {
    auth.require_ready()?;
    let c = crate::computers::load(&app, &auth, computer).await?;
    let prompt = b.prompt.trim();
    if prompt.is_empty() {
        return Err(ApiError::bad("Tell the agent what to do."));
    }
    let name: String = prompt.chars().take(60).collect();
    let setup = json!({ "project": b.project.trim_matches('/'), "prompt": prompt, "provider": "mock" });
    let job = crate::jobs::create_job(
        &app,
        &auth,
        &c,
        "code",
        "agent_task",
        &name,
        setup,
        &JobOptions { max_runtime_secs: Some(4 * 3600), ..Default::default() },
    )
    .await?;
    let run = crate::jobs::start_now(&app, &auth, &job, "manual", None).await?;
    crate::measure::funnel(&app.db, Some(auth.account.id), "agent_task_started", json!({})).await;
    Ok(Json(json!({ "job_id": job.id, "run_id": run })))
}

pub async fn tasks(
    State(app): State<AppState>,
    auth: Auth,
    Path(computer): Path<Uuid>,
    Query(q): Query<TreeQuery>,
) -> ApiResult<Json<Value>> {
    crate::computers::load(&app, &auth, computer).await?;
    let rows: Vec<(
        Uuid,
        String,
        Value,
        Option<Uuid>,
        Option<String>,
        Option<String>,
        Option<chrono::DateTime<chrono::Utc>>,
    )> = sqlx::query_as(
        "select j.id, j.name, j.setup, r.id, r.status, r.headline, r.queued_at from jobs j
         left join lateral (select * from runs where job_id = j.id order by queued_at desc limit 1) r on true
         where j.computer_id = $1 and j.kind = 'agent_task' and j.setup->>'project' = $2 and j.status <> 'deleted'
         order by j.created_at desc limit 50",
    )
    .bind(computer)
    .bind(q.project.trim_matches('/'))
    .fetch_all(&app.db)
    .await?;
    Ok(Json(
        json!({ "tasks": rows.into_iter().map(|(id, name, setup, run, status, headline, at)| json!({ "job_id": id, "name": name, "prompt": setup["prompt"], "run_id": run, "status": status, "headline": headline, "at": at })).collect::<Vec<_>>() }),
    ))
}

pub async fn review(State(app): State<AppState>, auth: Auth, Path(run): Path<Uuid>) -> ApiResult<Json<Value>> {
    let run = crate::jobs::load_run(&app, &auth, run).await?;
    let c = crate::computers::load(&app, &auth, run.computer_id).await?;
    crate::computers::ensure_awake(&app, &c, "open:code").await?;
    let (diffs, _): (Vec<FileDiff>, _) =
        app.relay.request(c.id, &StreamOpen::CodeReview(CodeReviewRequest::Diff { run_id: run.id }), None).await?;
    Ok(Json(json!({ "files": diffs })))
}

#[derive(Deserialize)]
pub struct Decision {
    pub path: String,
    /// keep or undo
    pub decision: String,
}

pub async fn decide(
    State(app): State<AppState>,
    auth: Auth,
    Path(run_id): Path<Uuid>,
    Json(b): Json<Decision>,
) -> ApiResult<Json<Value>> {
    let run = crate::jobs::load_run(&app, &auth, run_id).await?;
    let c = crate::computers::load(&app, &auth, run.computer_id).await?;
    crate::computers::ensure_awake(&app, &c, "open:code").await?;
    let req = match b.decision.as_str() {
        "keep" => CodeReviewRequest::Keep { run_id, path: b.path.clone() },
        "undo" => CodeReviewRequest::Undo { run_id, path: b.path.clone() },
        _ => return Err(ApiError::bad("Keep or undo?")),
    };
    let (res, _): (Value, _) = app.relay.request(c.id, &StreamOpen::CodeReview(req), None).await?;
    let project = run.data.as_ref().and_then(|d| d["project"].as_str()).unwrap_or_default();
    let path = format!("{project}/{}", b.path);
    let mut tx = app.db.begin().await?;
    sqlx::query("insert into file_records (computer_id, path, change, actor_kind, actor_id, run_id) values ($1, $2, $3, 'user', $4, $5)")
        .bind(c.id)
        .bind(&path)
        .bind(if b.decision == "keep" { "kept" } else { "undone" })
        .bind(auth.user.id)
        .bind(run_id)
        .execute(&mut *tx)
        .await?;
    crate::measure::audit(
        &mut *tx,
        auth.account.id,
        auth.actor_kind,
        Some(auth.user.id),
        &format!("agent.change_{}", b.decision),
        &path,
        json!({ "run_id": run_id }),
    )
    .await?;
    if res["pending"].as_i64() == Some(0) {
        sqlx::query(
            "update events set read_at = $2 where run_id = $1 and kind = 'agent.review_ready' and read_at is null",
        )
        .bind(run_id)
        .bind(app.now())
        .execute(&mut *tx)
        .await?;
    }
    crate::events::live(&mut *tx, auth.account.id, "run", &run_id.to_string(), json!({})).await?;
    tx.commit().await?;
    Ok(Json(res))
}

#[derive(Deserialize)]
pub struct DevServer {
    pub project: String,
    pub command: Option<String>,
    pub port: Option<u16>,
}

pub async fn start_dev_server(
    State(app): State<AppState>,
    auth: Auth,
    Path(computer): Path<Uuid>,
    Json(b): Json<DevServer>,
) -> ApiResult<Json<Value>> {
    let c = crate::computers::load(&app, &auth, computer).await?;
    let project = b.project.trim_matches('/').to_string();
    let port = b.port.unwrap_or(5173);
    if port < 1024 {
        return Err(ApiError::bad("Choose a port above 1023."));
    }
    let command = b
        .command
        .clone()
        .filter(|c| !c.trim().is_empty())
        .unwrap_or_else(|| "python3 -m http.server $PORT --bind 127.0.0.1".into());
    let setup = json!({ "project": project, "command": command, "port": port });
    let existing: Option<Job> = sqlx::query_as("select * from jobs where computer_id = $1 and kind = 'dev_server' and setup->>'project' = $2 and status <> 'deleted'")
        .bind(c.id)
        .bind(&project)
        .fetch_optional(&app.db)
        .await?;
    let job = match existing {
        Some(j) => {
            sqlx::query("update jobs set setup = $2 where id = $1").bind(j.id).bind(&setup).execute(&app.db).await?;
            let (running,): (i64,) = sqlx::query_as("select count(*) from runs where job_id = $1 and status in ('queued', 'waiting', 'starting', 'running')").bind(j.id).fetch_one(&app.db).await?;
            if running > 0 {
                return Err(ApiError::conflict("The dev server is already running."));
            }
            crate::jobs::load_job(&app, &auth, j.id).await?
        }
        None => {
            crate::jobs::create_job(
                &app,
                &auth,
                &c,
                "code",
                "dev_server",
                &format!("Dev server for {project}"),
                setup,
                &JobOptions { max_runtime_secs: Some(12 * 3600), ..Default::default() },
            )
            .await?
        }
    };
    let run = crate::jobs::start_now(&app, &auth, &job, "manual", None).await?;
    Ok(Json(json!({ "job_id": job.id, "run_id": run, "port": port })))
}

pub async fn dev_servers(
    State(app): State<AppState>,
    auth: Auth,
    Path(computer): Path<Uuid>,
    Query(q): Query<TreeQuery>,
) -> ApiResult<Json<Value>> {
    crate::computers::load(&app, &auth, computer).await?;
    let row: Option<(Uuid, Value, Option<Uuid>, Option<String>)> = sqlx::query_as(
        "select j.id, j.setup, r.id, r.status from jobs j left join lateral (select * from runs where job_id = j.id order by queued_at desc limit 1) r on true
         where j.computer_id = $1 and j.kind = 'dev_server' and j.setup->>'project' = $2 and j.status <> 'deleted'",
    )
    .bind(computer)
    .bind(q.project.trim_matches('/'))
    .fetch_optional(&app.db)
    .await?;
    let Some((job, setup, run, status)) = row else { return Ok(Json(json!({ "server": null }))) };
    let port = setup["port"].as_i64().unwrap_or(0);
    let (open,): (bool,) =
        sqlx::query_as("select exists(select 1 from open_ports where computer_id = $1 and port = $2)")
            .bind(computer)
            .bind(port as i32)
            .fetch_one(&app.db)
            .await?;
    Ok(Json(
        json!({ "server": { "job_id": job, "run_id": run, "status": status, "port": port, "command": setup["command"], "listening": open } }),
    ))
}

#[derive(Deserialize)]
pub struct PreviewBody {
    pub port: u16,
}

/// Mint a short-lived token for a preview of one port, on its own random subdomain.
pub async fn open_preview(
    State(app): State<AppState>,
    auth: Auth,
    Path(computer): Path<Uuid>,
    Json(b): Json<PreviewBody>,
) -> ApiResult<Json<Value>> {
    let c = crate::computers::load(&app, &auth, computer).await?;
    let existing: Option<(String,)> =
        sqlx::query_as("select id from previews where computer_id = $1 and user_id = $2 and port = $3")
            .bind(c.id)
            .bind(auth.user.id)
            .bind(b.port as i32)
            .fetch_optional(&app.db)
            .await?;
    let id = match existing {
        Some((id,)) => id,
        None => {
            let id = format!("p{}", &Uuid::new_v4().simple().to_string()[..16]);
            sqlx::query(
                "insert into previews (id, computer_id, account_id, user_id, port) values ($1, $2, $3, $4, $5)",
            )
            .bind(&id)
            .bind(c.id)
            .bind(auth.account.id)
            .bind(auth.user.id)
            .bind(b.port as i32)
            .execute(&app.db)
            .await?;
            id
        }
    };
    let token = crate::crypto::token();
    sqlx::query("insert into preview_tokens (token_hash, preview_id, kind, expires_at) values ($1, $2, 'open', $3)")
        .bind(crate::crypto::hash(&token))
        .bind(&id)
        .bind(app.real_now() + chrono::Duration::seconds(60))
        .execute(&app.db)
        .await?;
    crate::computers::touch_presence(&app, c.id, auth.user.id, "preview", Some("code")).await?;
    let base = format!("http://{id}.{}", app.cfg.preview_domain);
    Ok(Json(json!({ "url": format!("{base}/__croncave/open?token={token}"), "origin": base })))
}
