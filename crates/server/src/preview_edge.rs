//! Private previews, on their own domain with one random subdomain each, so code people
//! write never shares cookies or storage with the app. A short-lived token from the Code
//! app becomes a cookie scoped to that one preview; every request then travels to the
//! computer over its own outgoing connection.

use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::response::{Html, IntoResponse, Redirect, Response};
use croncave_proto::{HttpHead, HttpRequestHead, StreamOpen};

use crate::auth::cookie_value;
use crate::state::AppState;

const COOKIE: &str = "cc_preview";

pub fn router(app: AppState) -> Router {
    Router::new().fallback(handle).with_state(app)
}

fn preview_id(app: &AppState, headers: &HeaderMap) -> Option<String> {
    let host = headers.get(header::HOST)?.to_str().ok()?.to_lowercase();
    let (id, rest) = host.split_once('.')?;
    if rest != app.cfg.preview_domain.to_lowercase() && !app.cfg.preview_domain.starts_with(rest) {
        return None;
    }
    (id.len() == 17 && id.starts_with('p') && id[1..].chars().all(|c| c.is_ascii_hexdigit())).then(|| id.to_string())
}

fn page(status: StatusCode, title: &str, text: &str) -> Response {
    (status, Html(format!("<!doctype html><meta charset=utf-8><title>{title}</title><body style=\"font-family:system-ui;max-width:32rem;margin:4rem auto;padding:0 1rem\"><h1>{title}</h1><p>{text}</p></body>"))).into_response()
}

async fn handle(State(app): State<AppState>, req: Request) -> Response {
    let Some(id) = preview_id(&app, req.headers()) else {
        return page(StatusCode::NOT_FOUND, "Not a preview", "This address isn't a Croncave preview.");
    };
    let now = app.real_now();
    if req.uri().path() == "/__croncave/open" {
        let token =
            req.uri().query().and_then(|q| q.split('&').find_map(|kv| kv.strip_prefix("token="))).unwrap_or_default();
        let used: Option<(String,)> = sqlx::query_as(
            "update preview_tokens set used_at = $3 where token_hash = $1 and preview_id = $2 and kind = 'open' and used_at is null and expires_at > $3 returning preview_id",
        )
        .bind(crate::crypto::hash(token))
        .bind(&id)
        .bind(now)
        .fetch_optional(&app.db)
        .await
        .ok()
        .flatten();
        if used.is_none() {
            return page(
                StatusCode::UNAUTHORIZED,
                "This preview link expired",
                "Open the preview again from the Code app.",
            );
        }
        let session = crate::crypto::token();
        if sqlx::query(
            "insert into preview_tokens (token_hash, preview_id, kind, expires_at) values ($1, $2, 'session', $3)",
        )
        .bind(crate::crypto::hash(&session))
        .bind(&id)
        .bind(now + chrono::Duration::hours(12))
        .execute(&app.db)
        .await
        .is_err()
        {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
        let mut resp = Redirect::to("/").into_response();
        // SameSite=None so the preview also works inside the Code app's frame.
        let cookie = format!("{COOKIE}={session}; Path=/; HttpOnly; Secure; SameSite=None; Max-Age=43200");
        resp.headers_mut().append(header::SET_COOKIE, HeaderValue::from_str(&cookie).expect("valid cookie"));
        return resp;
    }

    let Some(session) = cookie_value(req.headers(), COOKIE) else {
        return page(
            StatusCode::UNAUTHORIZED,
            "This preview is private",
            "Only you can open it, from the Code app on Croncave.",
        );
    };
    let row: Option<(uuid::Uuid, uuid::Uuid, i32)> = sqlx::query_as(
        "select p.computer_id, p.user_id, p.port from preview_tokens t join previews p on p.id = t.preview_id
         where t.token_hash = $1 and t.preview_id = $2 and t.kind = 'session' and t.expires_at > $3",
    )
    .bind(crate::crypto::hash(&session))
    .bind(&id)
    .bind(now)
    .fetch_optional(&app.db)
    .await
    .ok()
    .flatten();
    let Some((computer_id, user_id, port)) = row else {
        return page(StatusCode::UNAUTHORIZED, "This preview is private", "Open it again from the Code app.");
    };
    let computer: Option<crate::model::Computer> =
        sqlx::query_as("select * from computers where id = $1 and deleted_at is null")
            .bind(computer_id)
            .fetch_optional(&app.db)
            .await
            .ok()
            .flatten();
    let Some(computer) = computer else { return page(StatusCode::NOT_FOUND, "Gone", "That computer was deleted.") };
    // A visit is a wake trigger, and an open preview keeps the computer awake.
    let _ = crate::computers::touch_presence(&app, computer_id, user_id, "preview", Some("preview")).await;
    let _ =
        sqlx::query("update previews set last_used_at = $2 where id = $1").bind(&id).bind(now).execute(&app.db).await;
    if let Err(e) = crate::computers::ensure_awake(&app, &computer, "preview").await {
        return page(StatusCode::SERVICE_UNAVAILABLE, "Your computer isn't awake", &e.message);
    }

    let method = req.method().clone();
    let path = req.uri().path_and_query().map(|p| p.as_str().to_string()).unwrap_or_else(|| "/".into());
    let headers: Vec<(String, String)> = req
        .headers()
        .iter()
        .filter(|(k, _)| *k != header::COOKIE || true)
        .filter_map(|(k, v)| {
            let value = v.to_str().ok()?.to_string();
            // Never pass the preview cookie to the person's own server.
            if k == header::COOKIE {
                let kept: Vec<&str> =
                    value.split(';').map(str::trim).filter(|c| !c.starts_with(&format!("{COOKIE}="))).collect();
                return (!kept.is_empty()).then(|| (k.to_string(), kept.join("; ")));
            }
            Some((k.to_string(), value))
        })
        .collect();
    let body = match axum::body::to_bytes(req.into_body(), 32 * 1024 * 1024).await {
        Ok(b) => b,
        Err(_) => return StatusCode::PAYLOAD_TOO_LARGE.into_response(),
    };
    let open =
        StreamOpen::PreviewHttp(HttpRequestHead { port: port as u16, method: method.to_string(), path, headers });
    let send_body = if method == Method::GET || method == Method::HEAD || body.is_empty() { None } else { Some(body) };
    let (head, mut stream): (HttpHead, _) = match app.relay.request(computer_id, &open, send_body).await {
        Ok(ok) => ok,
        Err(e) => return page(StatusCode::BAD_GATEWAY, "Couldn't reach your computer", &e.to_string()),
    };
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Bytes, std::io::Error>>(16);
    tokio::spawn(async move {
        while let Some(chunk) = stream.recv().await {
            let item = chunk.map_err(|e| std::io::Error::other(e.to_string()));
            let stop = item.is_err();
            if tx.send(item).await.is_err() || stop {
                break;
            }
        }
    });
    let mut resp = Response::new(Body::from_stream(tokio_stream::wrappers::ReceiverStream::new(rx)));
    *resp.status_mut() = StatusCode::from_u16(head.status).unwrap_or(StatusCode::BAD_GATEWAY);
    for (k, v) in head.headers {
        if let (Ok(k), Ok(v)) = (header::HeaderName::try_from(k.as_str()), HeaderValue::from_str(&v)) {
            if k == header::CONTENT_LENGTH {
                continue;
            }
            resp.headers_mut().append(k, v);
        }
    }
    resp
}
