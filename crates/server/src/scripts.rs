//! The Scripts app's control-plane side: set up script jobs, and starter scripts.

use axum::Json;
use axum::extract::{Path, State};
use bytes::Bytes;
use croncave_proto::{FilesRequest, Runtime, ScriptSetup, StreamOpen};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::auth::Auth;
use crate::error::{ApiError, ApiResult};
use crate::jobs::JobOptions;
use crate::state::AppState;

pub fn check_setup(s: &ScriptSetup) -> ApiResult<()> {
    if s.path.trim().is_empty() || s.path.contains("..") {
        return Err(ApiError::bad("Choose the script to run from your files."));
    }
    if s.runtime.is_none() && Runtime::for_path(&s.path).is_none() {
        return Err(ApiError::bad("Choose whether this is a Python, Node.js or shell script."));
    }
    Ok(())
}

#[derive(Deserialize)]
pub struct NewScript {
    pub name: String,
    pub path: String,
    pub runtime: Option<Runtime>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub secrets: Vec<crate::jobs::SecretBody>,
    #[serde(default)]
    pub run_now: bool,
    /// Try it once without counting it as a run (the Add screen's test run).
    #[serde(default)]
    pub test_now: bool,
    #[serde(default = "yes")]
    pub install: bool,
    #[serde(flatten)]
    pub opts: JobOptions,
}

fn yes() -> bool {
    true
}

pub async fn create(
    State(app): State<AppState>,
    auth: Auth,
    Path(computer): Path<Uuid>,
    Json(b): Json<NewScript>,
) -> ApiResult<Json<Value>> {
    auth.require_ready()?;
    let c = crate::computers::load(&app, &auth, computer).await?;
    let setup = ScriptSetup {
        path: b.path.trim().trim_start_matches('/').to_string(),
        runtime: b.runtime,
        args: b.args,
        install: b.install,
    };
    check_setup(&setup)?;
    let job = crate::jobs::create_job(
        &app,
        &auth,
        &c,
        "scripts",
        "script",
        &b.name,
        serde_json::to_value(&setup).expect("serializes"),
        &b.opts,
    )
    .await?;
    for s in &b.secrets {
        if !s.name.trim().is_empty() {
            let _saved = crate::jobs::set_secret(
                State(app.clone()),
                auth.clone(),
                Path(job.id),
                Json(crate::jobs::SecretBody { name: s.name.clone(), value: s.value.clone() }),
            )
            .await?;
        }
    }
    let run = if b.test_now {
        let mut setup = job.setup.clone();
        setup["test"] = json!(true);
        Some(crate::jobs::start_now(&app, &auth, &job, "test", Some(setup)).await?)
    } else if b.run_now {
        Some(crate::jobs::start_now(&app, &auth, &job, "manual", None).await?)
    } else {
        None
    };
    Ok(Json(json!({ "job": crate::jobs::job_view(&app, &job).await?, "run_id": run })))
}

/// Starter scripts, written into the person's files so they can read and change them.
pub fn templates() -> Vec<(&'static str, &'static str, &'static str, Vec<(&'static str, &'static str)>)> {
    vec![
        (
            "csv-report",
            "Summarize a spreadsheet (Python)",
            "Scripts/csv-report/report.py",
            vec![
                ("Scripts/csv-report/report.py", include_str!("../templates/csv-report/report.py")),
                ("Scripts/csv-report/sales.csv", include_str!("../templates/csv-report/sales.csv")),
            ],
        ),
        (
            "backup",
            "Back up a folder (shell)",
            "Scripts/backup/backup.sh",
            vec![("Scripts/backup/backup.sh", include_str!("../templates/backup/backup.sh"))],
        ),
        (
            "page-title",
            "Fetch a page's title (Node.js)",
            "Scripts/page-title/title.js",
            vec![("Scripts/page-title/title.js", include_str!("../templates/page-title/title.js"))],
        ),
    ]
}

pub async fn list_templates() -> Json<Value> {
    Json(
        json!({ "templates": templates().into_iter().map(|(id, label, entry, _)| json!({ "id": id, "label": label, "entry": entry })).collect::<Vec<_>>() }),
    )
}

#[derive(Deserialize)]
pub struct TemplateBody {
    pub template: String,
}

pub async fn add_template(
    State(app): State<AppState>,
    auth: Auth,
    Path(computer): Path<Uuid>,
    Json(b): Json<TemplateBody>,
) -> ApiResult<Json<Value>> {
    let c = crate::computers::load(&app, &auth, computer).await?;
    let (_, label, entry, files) =
        templates().into_iter().find(|t| t.0 == b.template).ok_or_else(|| ApiError::bad("Unknown starter script."))?;
    crate::computers::ensure_awake(&app, &c, "open:scripts").await?;
    let mut written = Vec::new();
    for (path, body) in files {
        write_file(&app, &c, path, Bytes::from_static(body.as_bytes())).await?;
        written.push(path.to_string());
    }
    crate::files_api::record_user_change(&app, &auth, c.id, &written, "created").await?;
    Ok(Json(json!({ "entry": entry, "label": label, "files": written })))
}

pub async fn write_file(app: &AppState, c: &crate::model::Computer, path: &str, body: Bytes) -> ApiResult<Value> {
    let (head, _s): (Value, _) =
        app.relay.request(c.id, &StreamOpen::Files(FilesRequest::Write { path: path.into() }), Some(body)).await?;
    Ok(head)
}
