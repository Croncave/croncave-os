//! The app registry. Each app is written by hand for its job; its manifest tells the
//! platform what it needs (permissions, job kinds, events, worker).

use axum::Json;
use serde_json::{Value, json};

use crate::auth::Auth;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

pub fn manifests() -> Value {
    json!([
        {
            "id": "files", "name": "Files", "version": "1", "built_in": true,
            "description": "Your computer's files: upload, browse, preview, Trash.",
            "permissions": ["files:all"], "job_kinds": [], "events": ["file.uploaded", "file.deleted"],
            "worker": { "kind": "built_in_agent" }
        },
        {
            "id": "scripts", "name": "Scripts", "version": "1",
            "description": "Run your own Python, Node.js and shell scripts by hand, on a schedule, or when files change.",
            "permissions": ["files:picked", "secrets:job", "network:egress"],
            "job_kinds": [{ "kind": "script", "label": "Script" }],
            "events": ["run.finished", "run.failed"],
            "worker": { "kind": "process", "runtimes": ["python", "node", "shell"] }
        },
        {
            "id": "watcher", "name": "Watcher", "version": "1",
            "description": "Watch pages, prices and listings, and hear about changes.",
            "permissions": ["app_data", "network:egress", "sdk:market.quote"],
            "job_kinds": [{ "kind": "check", "label": "Watch" }],
            "events": ["watch.found", "run.failed"],
            "worker": { "kind": "engine", "types": "config" }
        },
        {
            "id": "code", "name": "Code", "version": "1",
            "description": "Edit a project, hand a coding agent a task, review its changes, preview what it serves.",
            "permissions": ["files:picked", "app_data", "previews", "ai:own_sign_in"],
            "job_kinds": [{ "kind": "agent_task", "label": "Agent task" }, { "kind": "dev_server", "label": "Dev server" }],
            "events": ["agent.needs_approval", "agent.review_ready"],
            "worker": { "kind": "process", "hosts": ["mock coding agent"] }
        }
    ])
}

pub async fn list() -> Json<Value> {
    Json(json!({ "apps": manifests() }))
}

/// Check an app's job setup before it is saved.
pub async fn validate_setup(app: &AppState, auth: &Auth, app_id: &str, kind: &str, setup: Value) -> ApiResult<Value> {
    match (app_id, kind) {
        ("scripts", "script") => {
            let s: croncave_proto::ScriptSetup =
                serde_json::from_value(setup).map_err(|_| ApiError::bad("The script's setup isn't complete."))?;
            crate::scripts::check_setup(&s)?;
            Ok(serde_json::to_value(s).expect("serializes"))
        }
        ("watcher", "check") => {
            let type_id = setup
                .get("watch_type")
                .and_then(|t| t.get("id"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let inputs = setup.get("inputs").and_then(Value::as_object).cloned().unwrap_or_default();
            crate::watcher::build_setup(app, auth, &type_id, &inputs).await
        }
        ("code", _) => Ok(setup),
        _ => Err(ApiError::bad("Unknown app or job kind.")),
    }
}
