//! The Watcher app's control-plane side: watcher types (data the engine runs), setting up
//! watches from a type, testing them before they go live.

use axum::Json;
use axum::extract::{Path, State};
use croncave_proto::watcher::WatcherType;
use serde::Deserialize;
use serde_json::{Map, Value, json};
use uuid::Uuid;

use crate::auth::Auth;
use crate::error::{ApiError, ApiResult};
use crate::jobs::JobOptions;
use crate::model::Job;
use crate::state::AppState;

pub fn builtin_types() -> Vec<WatcherType> {
    [
        include_str!("../catalog/watcher-types/web-page.json"),
        include_str!("../catalog/watcher-types/page-contains.json"),
        include_str!("../catalog/watcher-types/demo-listings.json"),
        include_str!("../catalog/watcher-types/stock-price.json"),
    ]
    .iter()
    .map(|s| serde_json::from_str(s).expect("built-in watcher types are valid"))
    .collect()
}

/// Built-in demo types read the local demo sites; `{{demo}}` is where computers reach them
/// (the host itself for the local driver, the Docker bridge for containers).
fn with_demo(config: Value, demo: &str) -> Value {
    let text = config.to_string();
    if !text.contains("{{demo}}") {
        return config;
    }
    serde_json::from_str(&text.replace("{{demo}}", demo.trim_end_matches('/'))).unwrap_or(config)
}

/// The newest approved version of a type this account may use.
pub async fn load_type(app: &AppState, auth: &Auth, id: &str) -> ApiResult<WatcherType> {
    let row: Option<(Value,)> = sqlx::query_as(
        "select config from watcher_types where id = $1 and approved_at is not null and (account_id is null or account_id = $2)
         order by version desc limit 1",
    )
    .bind(id)
    .bind(auth.account.id)
    .fetch_optional(&app.db)
    .await?;
    let (config,) = row.ok_or_else(|| ApiError::not_found("That watch type"))?;
    serde_json::from_value(with_demo(config, &app.cfg.demo_url))
        .map_err(|_| ApiError::bad("That watch type is damaged."))
}

pub async fn build_setup(app: &AppState, auth: &Auth, type_id: &str, inputs: &Map<String, Value>) -> ApiResult<Value> {
    let t = load_type(app, auth, type_id).await?;
    let resolved = t.resolve_inputs(inputs).map_err(|p| ApiError::bad(p.join(" ")))?;
    Ok(json!({ "watch_type": t, "inputs": resolved }))
}

pub fn plain_rule(job: &Job) -> String {
    let t: Option<WatcherType> = job.setup.get("watch_type").and_then(|t| serde_json::from_value(t.clone()).ok());
    let inputs = job.setup.get("inputs").and_then(Value::as_object).cloned().unwrap_or_default();
    let rule = t.map(|t| t.plain_rule(&inputs)).unwrap_or_else(|| job.name.clone());
    match (&job.trigger[..], job.schedule.as_deref()) {
        ("schedule", Some(s)) => format!("{}, {}.", rule, crate::jobs::describe_schedule(s).to_lowercase()),
        _ => format!("{rule}, when I press Check now."),
    }
}

pub async fn types(State(app): State<AppState>, auth: Auth) -> ApiResult<Json<Value>> {
    let rows: Vec<(String, i32, Value, String, Option<chrono::DateTime<chrono::Utc>>)> = sqlx::query_as(
        "select distinct on (id) id, version, config, origin, approved_at from watcher_types
         where account_id is null or account_id = $1 order by id, version desc",
    )
    .bind(auth.account.id)
    .fetch_all(&app.db)
    .await?;
    let types: Vec<Value> = rows
        .into_iter()
        .map(|(id, version, config, origin, approved)| {
            let config = with_demo(config, &app.cfg.demo_url);
            let reads = match config.pointer("/source/type").and_then(Value::as_str) {
                Some("http") => match config.pointer("/source/url").and_then(Value::as_str).unwrap_or("") {
                    u if u.contains('{') => "Reads the web page you give it, from your computer".to_string(),
                    u => format!("Reads the web page at {u}"),
                },
                Some("platform") => "Reads prices from Croncave's market data".to_string(),
                _ => String::new(),
            };
            json!({ "id": id, "version": version, "config": config, "origin": origin, "approved": approved.is_some(), "reads": reads })
        })
        .collect();
    Ok(Json(json!({ "types": types })))
}

#[derive(Deserialize)]
pub struct NewType {
    pub config: Value,
}

/// A type someone made (or the assistant drafted). It waits for approval before it runs.
pub async fn create_type(State(app): State<AppState>, auth: Auth, Json(b): Json<NewType>) -> ApiResult<Json<Value>> {
    let t: WatcherType =
        serde_json::from_value(b.config.clone()).map_err(|e| ApiError::bad(format!("The type isn't complete: {e}")))?;
    let problems = t.problems();
    if !problems.is_empty() {
        return Err(ApiError::bad(problems.join(" ")));
    }
    let builtin = builtin_types().iter().any(|bt| bt.id == t.id);
    if builtin {
        return Err(ApiError::conflict("That id belongs to a built-in type. Choose another."));
    }
    let (version,): (i32,) = sqlx::query_as("select coalesce(max(version), 0) + 1 from watcher_types where id = $1")
        .bind(&t.id)
        .fetch_one(&app.db)
        .await?;
    let mut stored = t.clone();
    stored.version = version as u32;
    sqlx::query("insert into watcher_types (id, version, config, origin, account_id) values ($1, $2, $3, $4, $5)")
        .bind(&t.id)
        .bind(version)
        .bind(serde_json::to_value(&stored).expect("serializes"))
        .bind(if auth.actor_kind == "assistant" { "assistant" } else { "user" })
        .bind(auth.account.id)
        .execute(&app.db)
        .await?;
    Ok(Json(json!({ "id": t.id, "version": version, "approved": false })))
}

pub async fn approve_type(
    State(app): State<AppState>,
    auth: Auth,
    Path((id, version)): Path<(String, i32)>,
) -> ApiResult<Json<Value>> {
    if auth.actor_kind == "assistant" {
        return Err(ApiError::forbidden("Only you can approve a watch type."));
    }
    let r = sqlx::query("update watcher_types set approved_at = $4 where id = $1 and version = $2 and account_id = $3 and approved_at is null")
        .bind(&id)
        .bind(version)
        .bind(auth.account.id)
        .bind(app.now())
        .execute(&app.db)
        .await?;
    if r.rows_affected() == 0 {
        return Err(ApiError::not_found("That unapproved type"));
    }
    crate::measure::audit(
        &app.db,
        auth.account.id,
        "user",
        Some(auth.user.id),
        "watcher_type.approved",
        &id,
        json!({ "version": version }),
    )
    .await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct NewWatch {
    pub type_id: String,
    pub name: String,
    #[serde(default)]
    pub inputs: Map<String, Value>,
    #[serde(default)]
    pub check_now: bool,
    #[serde(flatten)]
    pub opts: JobOptions,
}

pub async fn create(
    State(app): State<AppState>,
    auth: Auth,
    Path(computer): Path<Uuid>,
    Json(b): Json<NewWatch>,
) -> ApiResult<Json<Value>> {
    auth.require_ready()?;
    let c = crate::computers::load(&app, &auth, computer).await?;
    let setup = build_setup(&app, &auth, &b.type_id, &b.inputs).await?;
    let job = crate::jobs::create_job(&app, &auth, &c, "watcher", "check", &b.name, setup, &b.opts).await?;
    let run = if b.check_now && job.status == "active" {
        Some(crate::jobs::start_now(&app, &auth, &job, "manual", None).await?)
    } else {
        None
    };
    let mut v = crate::jobs::job_view(&app, &job).await?;
    v["rule"] = json!(plain_rule(&job));
    Ok(Json(json!({ "job": v, "run_id": run })))
}

#[derive(Deserialize)]
pub struct Preview {
    pub type_id: String,
    #[serde(default)]
    pub inputs: Map<String, Value>,
    #[serde(flatten)]
    pub opts: JobOptions,
}

/// The setup in plain words, before saving.
pub async fn describe(State(app): State<AppState>, auth: Auth, Json(b): Json<Preview>) -> ApiResult<Json<Value>> {
    let t = load_type(&app, &auth, &b.type_id).await?;
    let problems = t.resolve_inputs(&b.inputs).err().unwrap_or_default();
    let rule = t.plain_rule(&b.inputs);
    let when = match (b.opts.trigger.as_deref(), b.opts.schedule.as_deref()) {
        (Some("schedule"), Some(s)) => crate::jobs::parse_schedule(s)
            .map(|(six, _)| crate::jobs::describe_schedule(&six).to_lowercase())
            .unwrap_or_else(|e| e),
        _ => "when I press Check now".into(),
    };
    Ok(Json(json!({ "rule": format!("{rule}, {when}."), "problems": problems })))
}

/// Test a job's setup on the computer: real results, nothing saved, no one told.
pub async fn test(State(app): State<AppState>, auth: Auth, Path(job): Path<Uuid>) -> ApiResult<Json<Value>> {
    let job = crate::jobs::load_job(&app, &auth, job).await?;
    let mut setup = job.setup.clone();
    setup["test"] = json!(true);
    let run = crate::jobs::start_now(&app, &auth, &job, "test", Some(setup)).await?;
    Ok(Json(json!({ "run_id": run })))
}
