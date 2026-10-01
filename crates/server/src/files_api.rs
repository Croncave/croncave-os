//! The Files app's API. Everything travels over the computer's own connection; opening
//! Files wakes the computer and keeps it awake while the view is open.

use axum::Json;
use axum::body::{Body, Bytes};
use axum::extract::{Path, Query, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use base64::Engine;
use croncave_proto::{DiskUsage, FileEntry, FilePreview, FilesRequest, StreamOpen, TrashEntry};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::auth::Auth;
use crate::error::{ApiError, ApiResult};
use crate::model::Computer;
use crate::state::AppState;

async fn ready(app: &AppState, auth: &Auth, id: Uuid) -> ApiResult<Computer> {
    let c = crate::computers::load(app, auth, id).await?;
    crate::computers::touch_presence(app, c.id, auth.user.id, "files", Some("files")).await?;
    crate::computers::ensure_awake(app, &c, "open:files").await?;
    Ok(c)
}

async fn ask<T: serde::de::DeserializeOwned>(
    app: &AppState,
    c: &Computer,
    req: FilesRequest,
    body: Option<Bytes>,
) -> ApiResult<T> {
    let (head, _s) = app.relay.request(c.id, &StreamOpen::Files(req), body).await?;
    Ok(head)
}

/// Record that the person changed files (for source tags and file triggers).
pub async fn record_user_change(
    app: &AppState,
    auth: &Auth,
    computer: Uuid,
    paths: &[String],
    change: &str,
) -> ApiResult<()> {
    let mut tx = app.db.begin().await?;
    for p in paths {
        sqlx::query("insert into file_records (computer_id, path, change, actor_kind, actor_id, at) values ($1, $2, $3, $4, $5, $6)")
            .bind(computer)
            .bind(p)
            .bind(change)
            .bind(auth.actor_kind)
            .bind(auth.user.id)
            .bind(app.now())
            .execute(&mut *tx)
            .await?;
    }
    if change != "deleted" {
        crate::relay_hooks::queue_file_triggers(app, &mut tx, computer, None, paths).await?;
    }
    tx.commit().await?;
    app.kick.notify_waiters();
    Ok(())
}

#[derive(Deserialize)]
pub struct PathQuery {
    #[serde(default)]
    pub path: String,
}

/// Each entry with where it came from: the latest record for its path.
async fn with_sources(app: &AppState, computer: Uuid, entries: Vec<FileEntry>) -> ApiResult<Vec<Value>> {
    let paths: Vec<String> = entries.iter().map(|e| e.path.clone()).collect();
    let sources: Vec<(String, String, String, Option<Uuid>, Option<String>, chrono::DateTime<chrono::Utc>)> =
        sqlx::query_as(
            "select distinct on (f.path) f.path, f.change, f.actor_kind, f.run_id, j.name, f.at
         from file_records f left join runs r on r.id = f.run_id left join jobs j on j.id = r.job_id
         where f.computer_id = $1 and f.path = any($2) order by f.path, f.id desc",
        )
        .bind(computer)
        .bind(&paths)
        .fetch_all(&app.db)
        .await?;
    let items: Vec<Value> = entries
        .into_iter()
        .map(|e| {
            let src = sources.iter().find(|s| s.0 == e.path).map(|(_, change, actor, run, job, at)| {
                let who = match (actor.as_str(), job) {
                    ("run", Some(j)) => format!("Made by \"{j}\""),
                    ("agent", Some(j)) => format!("Changed by the agent: {j}"),
                    ("assistant", _) => "Added by the assistant".to_string(),
                    _ => {
                        if change == "created" {
                            "Uploaded by you".to_string()
                        } else {
                            "Changed by you".to_string()
                        }
                    }
                };
                json!({ "label": who, "change": change, "actor": actor, "run_id": run, "at": at })
            });
            let mut v = serde_json::to_value(&e).unwrap_or_default();
            v["source"] = src.unwrap_or(Value::Null);
            v
        })
        .collect();
    Ok(items)
}

pub async fn list(
    State(app): State<AppState>,
    auth: Auth,
    Path(id): Path<Uuid>,
    Query(q): Query<PathQuery>,
) -> ApiResult<Json<Value>> {
    let c = ready(&app, &auth, id).await?;
    let entries: Vec<FileEntry> = ask(&app, &c, FilesRequest::List { path: q.path.clone() }, None).await?;
    let items = with_sources(&app, c.id, entries).await?;
    Ok(Json(json!({ "path": q.path, "entries": items })))
}

pub async fn preview(
    State(app): State<AppState>,
    auth: Auth,
    Path(id): Path<Uuid>,
    Query(q): Query<PathQuery>,
) -> ApiResult<Json<Value>> {
    let c = ready(&app, &auth, id).await?;
    let (head, mut s): (FilePreview, _) =
        app.relay.request(c.id, &StreamOpen::Files(FilesRequest::Preview { path: q.path.clone() }), None).await?;
    let mut v = serde_json::to_value(&head).unwrap_or_default();
    if matches!(head, FilePreview::Image { .. }) {
        let png = s.read_to_end().await?;
        v["data_url"] =
            json!(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png)));
    }
    Ok(Json(v))
}

pub async fn download(
    State(app): State<AppState>,
    auth: Auth,
    Path(id): Path<Uuid>,
    Query(q): Query<PathQuery>,
) -> ApiResult<Response> {
    let c = ready(&app, &auth, id).await?;
    let (head, mut s): (Value, _) =
        app.relay.request(c.id, &StreamOpen::Files(FilesRequest::Download { path: q.path.clone() }), None).await?;
    let name = head["name"].as_str().unwrap_or("download").replace('"', "");
    let stream = async_stream(move |tx| async move {
        while let Some(chunk) = s.recv().await {
            match chunk {
                Ok(b) => {
                    if tx.send(Ok::<_, std::io::Error>(b)).await.is_err() {
                        break;
                    }
                }
                Err(e) => {
                    let _ = tx.send(Err(std::io::Error::other(e.to_string()))).await;
                    break;
                }
            }
        }
    });
    Ok((
        [
            (header::CONTENT_TYPE, "application/octet-stream".to_string()),
            (header::CONTENT_DISPOSITION, format!("attachment; filename=\"{name}\"")),
            (header::CONTENT_LENGTH, head["size"].as_u64().unwrap_or(0).to_string()),
        ],
        Body::from_stream(stream),
    )
        .into_response())
}

fn async_stream<F, Fut>(f: F) -> tokio_stream::wrappers::ReceiverStream<Result<Bytes, std::io::Error>>
where
    F: FnOnce(tokio::sync::mpsc::Sender<Result<Bytes, std::io::Error>>) -> Fut,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    let (tx, rx) = tokio::sync::mpsc::channel(16);
    tokio::spawn(f(tx));
    tokio_stream::wrappers::ReceiverStream::new(rx)
}

#[derive(Deserialize)]
pub struct PathBody {
    pub path: String,
}

pub async fn mkdir(
    State(app): State<AppState>,
    auth: Auth,
    Path(id): Path<Uuid>,
    Json(b): Json<PathBody>,
) -> ApiResult<Json<FileEntry>> {
    let c = ready(&app, &auth, id).await?;
    Ok(Json(ask(&app, &c, FilesRequest::Mkdir { path: b.path }, None).await?))
}

#[derive(Deserialize)]
pub struct MoveBody {
    pub from: String,
    pub to: String,
}

pub async fn rename(
    State(app): State<AppState>,
    auth: Auth,
    Path(id): Path<Uuid>,
    Json(b): Json<MoveBody>,
) -> ApiResult<Json<FileEntry>> {
    let c = ready(&app, &auth, id).await?;
    let e: FileEntry = ask(&app, &c, FilesRequest::Move { from: b.from.clone(), to: b.to.clone() }, None).await?;
    record_user_change(&app, &auth, c.id, std::slice::from_ref(&e.path), "changed").await?;
    Ok(Json(e))
}

pub async fn delete(
    State(app): State<AppState>,
    auth: Auth,
    Path(id): Path<Uuid>,
    Json(b): Json<PathBody>,
) -> ApiResult<Json<TrashEntry>> {
    let c = ready(&app, &auth, id).await?;
    let t: TrashEntry =
        ask(&app, &c, FilesRequest::Delete { path: b.path.clone(), by: auth.actor_kind.to_string() }, None).await?;
    record_user_change(&app, &auth, c.id, std::slice::from_ref(&b.path), "deleted").await?;
    Ok(Json(t))
}

pub async fn trash(State(app): State<AppState>, auth: Auth, Path(id): Path<Uuid>) -> ApiResult<Json<Value>> {
    let c = ready(&app, &auth, id).await?;
    let items: Vec<TrashEntry> = ask(&app, &c, FilesRequest::TrashList, None).await?;
    // Name the run behind deletes by scripts and agents.
    let mut out = vec![];
    for t in items {
        let mut v = serde_json::to_value(&t).unwrap_or_default();
        if let Some(run) = t.deleted_by.strip_prefix("run:").and_then(|r| r.parse::<Uuid>().ok()) {
            let name: Option<(String,)> =
                sqlx::query_as("select j.name from runs r join jobs j on j.id = r.job_id where r.id = $1")
                    .bind(run)
                    .fetch_optional(&app.db)
                    .await?;
            v["deleted_by_label"] =
                json!(format!("Deleted by \"{}\"", name.map(|n| n.0).unwrap_or_else(|| "a run".into())));
            v["run_id"] = json!(run);
        } else {
            v["deleted_by_label"] =
                json!(if t.deleted_by == "assistant" { "Deleted by the assistant" } else { "Deleted by you" });
        }
        // Trash keeps things this long (the agent's CRONCAVE_TRASH_DAYS), then lets go.
        let age_days = (app.real_now().timestamp_millis() - t.deleted_ms).max(0) / 86_400_000;
        v["days_left"] = json!((TRASH_DAYS - age_days).max(0));
        out.push(v);
    }
    Ok(Json(json!({ "items": out, "keep_days": TRASH_DAYS })))
}

/// How long Trash keeps things before they're gone for good.
pub const TRASH_DAYS: i64 = 30;

#[derive(Deserialize)]
pub struct LimitQuery {
    pub limit: Option<usize>,
}

/// The most recently changed files on the computer, with where each came from.
pub async fn recent(
    State(app): State<AppState>,
    auth: Auth,
    Path(id): Path<Uuid>,
    Query(q): Query<LimitQuery>,
) -> ApiResult<Json<Value>> {
    let c = ready(&app, &auth, id).await?;
    let entries: Vec<FileEntry> = ask(&app, &c, FilesRequest::Recent { limit: q.limit.unwrap_or(50) }, None).await?;
    let items = with_sources(&app, c.id, entries).await?;
    Ok(Json(json!({ "entries": items })))
}

#[derive(Deserialize)]
pub struct CopyBody {
    pub path: String,
    pub to_computer: Uuid,
    /// Where on the other computer; the same path when left out.
    pub to_path: Option<String>,
}

/// Copy a file or folder to another of your computers. It travels through the control
/// plane in chunks (folders as a zip that the other computer unpacks); whatever was at
/// the destination goes to that computer's Trash, never overwritten.
pub async fn copy_to(
    State(app): State<AppState>,
    auth: Auth,
    Path(id): Path<Uuid>,
    Json(b): Json<CopyBody>,
) -> ApiResult<Json<Value>> {
    if b.to_computer == id {
        return Err(ApiError::bad("Choose another computer to copy to."));
    }
    let src = ready(&app, &auth, id).await?;
    let dst = ready(&app, &auth, b.to_computer).await?;
    let (head, mut s): (Value, _) =
        app.relay.request(src.id, &StreamOpen::Files(FilesRequest::Download { path: b.path.clone() }), None).await?;
    let size = head["size"].as_u64().unwrap_or(0);
    if size > dst.disk_gb as u64 * 1_000_000_000 {
        return Err(ApiError::bad(format!("That's bigger than {}'s storage. Add storage in its settings.", dst.name)));
    }
    let zipped = head["zip"].as_bool() == Some(true);
    let upload_id = Uuid::new_v4().simple().to_string();
    let mut offset = 0u64;
    let mut buf: Vec<u8> = Vec::with_capacity(1 << 20);
    loop {
        let chunk = s.recv().await;
        let done = chunk.is_none();
        if let Some(c) = chunk {
            buf.extend_from_slice(&c.map_err(|e| ApiError::unavailable(format!("The copy was interrupted: {e}")))?);
        }
        if buf.len() >= 1 << 20 || (done && !buf.is_empty()) {
            let r: Value = ask(
                &app,
                &dst,
                FilesRequest::UploadChunk { upload_id: upload_id.clone(), offset },
                Some(Bytes::from(std::mem::take(&mut buf))),
            )
            .await?;
            if r["mismatch"].as_bool() == Some(true) {
                return Err(ApiError::unavailable("The copy was interrupted. Try again."));
            }
            offset = r["received"].as_u64().unwrap_or(offset);
        }
        if done {
            break;
        }
    }
    let to = b.to_path.clone().unwrap_or_else(|| b.path.clone());
    let e: Value =
        ask(&app, &dst, FilesRequest::UploadFinish { upload_id, path: to.clone(), extract: zipped }, None).await?;
    record_user_change(&app, &auth, dst.id, &[e["path"].as_str().unwrap_or(&to).to_string()], "created").await?;
    Ok(Json(json!({ "entry": e, "computer": dst.name })))
}

#[derive(Deserialize)]
pub struct RestoreBody {
    pub trash_id: String,
}

pub async fn restore(
    State(app): State<AppState>,
    auth: Auth,
    Path(id): Path<Uuid>,
    Json(b): Json<RestoreBody>,
) -> ApiResult<Json<FileEntry>> {
    let c = ready(&app, &auth, id).await?;
    let e: FileEntry = ask(&app, &c, FilesRequest::Restore { trash_id: b.trash_id }, None).await?;
    record_user_change(&app, &auth, c.id, std::slice::from_ref(&e.path), "restored").await?;
    Ok(Json(e))
}

pub async fn empty_trash(State(app): State<AppState>, auth: Auth, Path(id): Path<Uuid>) -> ApiResult<Json<Value>> {
    let c = ready(&app, &auth, id).await?;
    Ok(Json(ask(&app, &c, FilesRequest::EmptyTrash, None).await?))
}

pub async fn usage(State(app): State<AppState>, auth: Auth, Path(id): Path<Uuid>) -> ApiResult<Json<DiskUsage>> {
    let c = ready(&app, &auth, id).await?;
    Ok(Json(ask(&app, &c, FilesRequest::Usage, None).await?))
}

#[derive(Deserialize)]
pub struct NewUpload {
    pub path: String,
    pub size: u64,
}

pub async fn start_upload(
    State(app): State<AppState>,
    auth: Auth,
    Path(id): Path<Uuid>,
    Json(b): Json<NewUpload>,
) -> ApiResult<Json<Value>> {
    let c = ready(&app, &auth, id).await?;
    if b.path.trim().is_empty() || b.path.contains("..") {
        return Err(ApiError::bad("Choose where to put the file."));
    }
    let disk_bytes = c.disk_gb as u64 * 1_000_000_000;
    if b.size > disk_bytes {
        return Err(ApiError::bad("That file is bigger than this computer's storage. Add storage in its settings."));
    }
    Ok(Json(json!({ "upload_id": Uuid::new_v4().simple().to_string(), "chunk_size": 1024 * 1024 })))
}

#[derive(Deserialize)]
pub struct ChunkQuery {
    pub offset: u64,
}

/// One chunk of a resumable upload. A mismatch answers with what the computer really has.
pub async fn upload_chunk(
    State(app): State<AppState>,
    auth: Auth,
    Path((id, upload)): Path<(Uuid, String)>,
    Query(q): Query<ChunkQuery>,
    body: Bytes,
) -> ApiResult<Json<Value>> {
    let c = ready(&app, &auth, id).await?;
    Ok(Json(ask(&app, &c, FilesRequest::UploadChunk { upload_id: upload, offset: q.offset }, Some(body)).await?))
}

pub async fn upload_status(
    State(app): State<AppState>,
    auth: Auth,
    Path((id, upload)): Path<(Uuid, String)>,
) -> ApiResult<Json<Value>> {
    let c = ready(&app, &auth, id).await?;
    Ok(Json(ask(&app, &c, FilesRequest::UploadStatus { upload_id: upload }, None).await?))
}

pub async fn finish_upload(
    State(app): State<AppState>,
    auth: Auth,
    Path((id, upload)): Path<(Uuid, String)>,
    Json(b): Json<PathBody>,
) -> ApiResult<Json<Value>> {
    let c = ready(&app, &auth, id).await?;
    let e: Value =
        ask(&app, &c, FilesRequest::UploadFinish { upload_id: upload, path: b.path.clone(), extract: false }, None)
            .await?;
    let path = e["path"].as_str().unwrap_or(&b.path).to_string();
    record_user_change(
        &app,
        &auth,
        c.id,
        &[path],
        if e["replaced"].as_bool() == Some(true) { "changed" } else { "created" },
    )
    .await?;
    crate::measure::funnel(&app.db, Some(auth.account.id), "file_uploaded", json!({ "size": e["size"] })).await;
    Ok(Json(e))
}

/// Save a file (the Code editor).
pub async fn write(
    State(app): State<AppState>,
    auth: Auth,
    Path(id): Path<Uuid>,
    Query(q): Query<PathQuery>,
    body: Bytes,
) -> ApiResult<Json<Value>> {
    let c = ready(&app, &auth, id).await?;
    let e: Value = ask(&app, &c, FilesRequest::Write { path: q.path.clone() }, Some(body)).await?;
    record_user_change(
        &app,
        &auth,
        c.id,
        &[e["path"].as_str().unwrap_or(&q.path).to_string()],
        if e["created"].as_bool() == Some(true) { "created" } else { "changed" },
    )
    .await?;
    Ok(Json(e))
}

/// What a run made: its file changes.
pub async fn run_files(State(app): State<AppState>, auth: Auth, Path(run): Path<Uuid>) -> ApiResult<Json<Value>> {
    let r = crate::jobs::load_run(&app, &auth, run).await?;
    Ok(Json(json!({ "changes": r.changes.unwrap_or(json!([])) })))
}
