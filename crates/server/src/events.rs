//! Everything that happens becomes an event in one append-only table. Activity, Home,
//! notifications and the live UI all read from it.

use std::convert::Infallible;
use std::time::Duration;

use axum::Json;
use axum::extract::{Query, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use futures::Stream;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::postgres::PgListener;
use uuid::Uuid;

use crate::auth::Auth;
use crate::error::ApiResult;
use crate::state::{AppState, LiveMsg};

pub const LIVE_CHANNEL: &str = "cc_live";

#[derive(Debug, Clone)]
pub struct NewEvent {
    pub account_id: Uuid,
    pub computer_id: Option<Uuid>,
    pub actor_kind: &'static str,
    pub actor_id: Option<Uuid>,
    pub app: String,
    pub kind: String,
    /// info, success, needs_you, failed, warning, or system (shown only on request).
    pub level: &'static str,
    pub title: String,
    pub body: String,
    pub data: Value,
    pub run_id: Option<Uuid>,
    pub job_id: Option<Uuid>,
    /// Worth a notification on the channels the person turned on.
    pub notify: bool,
    /// Goes out even in quiet hours.
    pub urgent: bool,
}

impl NewEvent {
    pub fn new(account_id: Uuid, app: &str, kind: &str, level: &'static str, title: impl Into<String>) -> Self {
        Self {
            account_id,
            computer_id: None,
            actor_kind: "system",
            actor_id: None,
            app: app.into(),
            kind: kind.into(),
            level,
            title: title.into(),
            body: String::new(),
            data: json!({}),
            run_id: None,
            job_id: None,
            notify: false,
            urgent: false,
        }
    }
    pub fn computer(mut self, id: Uuid) -> Self {
        self.computer_id = Some(id);
        self
    }
    pub fn actor(mut self, kind: &'static str, id: Option<Uuid>) -> Self {
        self.actor_kind = kind;
        self.actor_id = id;
        self
    }
    pub fn body(mut self, b: impl Into<String>) -> Self {
        self.body = b.into();
        self
    }
    pub fn data(mut self, d: Value) -> Self {
        self.data = d;
        self
    }
    pub fn run(mut self, run: Uuid, job: Uuid) -> Self {
        self.run_id = Some(run);
        self.job_id = Some(job);
        self
    }
    pub fn notify(mut self) -> Self {
        self.notify = true;
        self
    }
    pub fn urgent(mut self) -> Self {
        self.notify = true;
        self.urgent = true;
        self
    }
}

/// Record an event and tell live subscribers, in one statement (so inside a transaction
/// the notification goes out only on commit).
pub async fn emit<'e, E: sqlx::PgExecutor<'e>>(ex: E, ev: NewEvent) -> sqlx::Result<i64> {
    let (id,): (i64,) = sqlx::query_as(
        "with e as (
            insert into events (account_id, computer_id, actor_kind, actor_id, app, kind, level, title, body, data,
                                run_id, job_id, notify, urgent, read_at)
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14,
                    case when $7 = 'system' then now() end)
            returning id, account_id, kind, level, title
         )
         select e.id from e, lateral (select pg_notify($15, json_build_object(
            'account_id', e.account_id, 'kind', 'event', 'id', e.id::text,
            'data', json_build_object('kind', e.kind, 'level', e.level, 'title', e.title))::text)) n",
    )
    .bind(ev.account_id)
    .bind(ev.computer_id)
    .bind(ev.actor_kind)
    .bind(ev.actor_id)
    .bind(&ev.app)
    .bind(&ev.kind)
    .bind(ev.level)
    .bind(&ev.title)
    .bind(&ev.body)
    .bind(&ev.data)
    .bind(ev.run_id)
    .bind(ev.job_id)
    .bind(ev.notify)
    .bind(ev.urgent)
    .bind(LIVE_CHANNEL)
    .fetch_one(ex)
    .await?;
    Ok(id)
}

/// Tell live subscribers that something changed (a run, a computer, output...).
pub async fn live<'e, E: sqlx::PgExecutor<'e>>(
    ex: E,
    account: Uuid,
    kind: &str,
    id: &str,
    data: Value,
) -> sqlx::Result<()> {
    let mut payload = json!({ "account_id": account, "kind": kind, "id": id, "data": data }).to_string();
    if payload.len() > 7000 {
        payload = json!({ "account_id": account, "kind": kind, "id": id, "data": {} }).to_string();
    }
    sqlx::query("select pg_notify($1, $2)").bind(LIVE_CHANNEL).bind(payload).execute(ex).await?;
    Ok(())
}

/// Forward Postgres notifications to in-process subscribers, reconnecting as needed.
pub async fn listen(app: AppState) {
    loop {
        match PgListener::connect_with(&app.db).await {
            Ok(mut l) => {
                if let Err(e) = l.listen(LIVE_CHANNEL).await {
                    tracing::warn!(error = %e, "listen failed");
                } else {
                    while let Ok(n) = l.recv().await {
                        if let Ok(msg) = serde_json::from_str::<LiveMsg>(n.payload()) {
                            let _ = app.live.send(msg);
                        }
                    }
                }
            }
            Err(e) => tracing::warn!(error = %e, "live listener could not connect"),
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

/// `GET /api/live`: server-sent events for the signed-in account.
pub async fn sse(State(app): State<AppState>, auth: Auth) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let mut rx = app.live.subscribe();
    let account = auth.account.id;
    let stream = async_stream(move |tx| async move {
        let _ = tx.send(Event::default().event("hello").data("{}")).await;
        loop {
            match rx.recv().await {
                Ok(msg) if msg.account_id == account => {
                    let data = serde_json::to_string(&msg).unwrap_or_default();
                    if tx.send(Event::default().event("live").data(data)).await.is_err() {
                        break;
                    }
                }
                Ok(_) => {}
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    let _ = tx.send(Event::default().event("resync").data("{}")).await;
                }
                Err(_) => break,
            }
        }
    });
    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}

/// A tiny channel-backed stream, to avoid pulling in a stream macro crate.
fn async_stream<F, Fut>(f: F) -> impl Stream<Item = Result<Event, Infallible>>
where
    F: FnOnce(tokio::sync::mpsc::Sender<Event>) -> Fut,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    let (tx, rx) = tokio::sync::mpsc::channel(64);
    tokio::spawn(f(tx));
    futures::StreamExt::map(tokio_stream::wrappers::ReceiverStream::new(rx), Ok)
}

#[derive(Deserialize)]
pub struct ListQuery {
    #[serde(default)]
    pub unread: bool,
    #[serde(default)]
    pub system: bool,
    pub before: Option<i64>,
    pub computer: Option<Uuid>,
    pub limit: Option<i64>,
}

#[derive(serde::Serialize, sqlx::FromRow)]
pub struct EventRow {
    pub id: i64,
    pub computer_id: Option<Uuid>,
    pub actor_kind: String,
    pub app: String,
    pub kind: String,
    pub level: String,
    pub title: String,
    pub body: String,
    pub data: Value,
    pub run_id: Option<Uuid>,
    pub job_id: Option<Uuid>,
    pub read_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn list(State(app): State<AppState>, auth: Auth, Query(q): Query<ListQuery>) -> ApiResult<Json<Value>> {
    let rows: Vec<EventRow> = sqlx::query_as(
        "select id, computer_id, actor_kind, app, kind, level, title, body, data, run_id, job_id, read_at, created_at
         from events
         where account_id = $1
           and ($2 = false or read_at is null)
           and ($3 = true or level <> 'system')
           and ($4::bigint is null or id < $4)
           and ($5::uuid is null or computer_id = $5)
         order by id desc limit $6",
    )
    .bind(auth.account.id)
    .bind(q.unread)
    .bind(q.system)
    .bind(q.before)
    .bind(q.computer)
    .bind(q.limit.unwrap_or(50).clamp(1, 200))
    .fetch_all(&app.db)
    .await?;
    let unread = unread_count(&app, auth.account.id).await?;
    Ok(Json(json!({ "events": rows, "unread": unread })))
}

pub async fn unread_count(app: &AppState, account: Uuid) -> sqlx::Result<i64> {
    let (n,): (i64,) =
        sqlx::query_as("select count(*) from events where account_id = $1 and read_at is null and level <> 'system'")
            .bind(account)
            .fetch_one(&app.db)
            .await?;
    Ok(n)
}

#[derive(Deserialize)]
pub struct ReadBody {
    #[serde(default)]
    pub ids: Vec<i64>,
    #[serde(default)]
    pub all: bool,
}

pub async fn mark_read(State(app): State<AppState>, auth: Auth, Json(b): Json<ReadBody>) -> ApiResult<Json<Value>> {
    sqlx::query("update events set read_at = $3 where account_id = $1 and read_at is null and ($2 or id = any($4))")
        .bind(auth.account.id)
        .bind(b.all)
        .bind(app.now())
        .bind(&b.ids)
        .execute(&app.db)
        .await?;
    live(&app.db, auth.account.id, "read", "", json!({})).await?;
    Ok(Json(json!({ "unread": unread_count(&app, auth.account.id).await? })))
}
