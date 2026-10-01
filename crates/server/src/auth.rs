//! Sign-in with an email link (no password), phone verification by text message, and
//! sessions. The links and codes go through the Notifier, so in the prototype they are
//! read on the Dev tools outbox page.

use axum::Json;
use axum::extract::{FromRequestParts, State};
use axum::http::header::{COOKIE, SET_COOKIE};
use axum::http::request::Parts;
use axum::http::{HeaderMap, HeaderValue};
use axum::response::{IntoResponse, Response};
use chrono::Duration;
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::crypto;
use crate::error::{ApiError, ApiResult};
use crate::model::{Account, User};
use crate::providers::notifier::{Channel, Message};
use crate::state::AppState;

pub const SESSION_COOKIE: &str = "cc_session";

/// The signed-in person, their account, and who is acting (the person or the assistant
/// on their behalf).
#[derive(Debug, Clone)]
pub struct Auth {
    pub user: User,
    pub account: Account,
    pub actor_kind: &'static str,
}

impl Auth {
    /// Paths that need a finished sign-up.
    pub fn require_ready(&self) -> ApiResult<()> {
        if self.account.signup_completed_at.is_none() {
            return Err(ApiError::forbidden("Finish signing up first.").with_code("needs_signup"));
        }
        Ok(())
    }
}

pub fn cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers.get_all(COOKIE).iter().filter_map(|v| v.to_str().ok()).flat_map(|v| v.split(';')).find_map(|kv| {
        let (k, v) = kv.trim().split_once('=')?;
        (k == name).then(|| v.to_string())
    })
}

impl FromRequestParts<AppState> for Auth {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, app: &AppState) -> Result<Self, Self::Rejection> {
        let token = cookie_value(&parts.headers, SESSION_COOKIE).ok_or_else(ApiError::unauthorized)?;
        let user: Option<User> = sqlx::query_as(
            "select u.* from sessions s join users u on u.id = s.user_id where s.token_hash = $1 and s.expires_at > $2",
        )
        .bind(crypto::hash(&token))
        .bind(app.real_now())
        .fetch_optional(&app.db)
        .await?;
        let user = user.ok_or_else(ApiError::unauthorized)?;
        let account: Account = sqlx::query_as(
            "select a.* from accounts a join account_members m on m.account_id = a.id where m.user_id = $1 order by a.created_at limit 1",
        )
        .bind(user.id)
        .fetch_one(&app.db)
        .await?;
        let actor_kind = match parts.headers.get("x-croncave-actor").and_then(|v| v.to_str().ok()) {
            Some("assistant") => "assistant",
            _ => "user",
        };
        Ok(Auth { user, account, actor_kind })
    }
}

/// An admin; everyone else gets 403.
pub struct Admin(pub Auth);

impl FromRequestParts<AppState> for Admin {
    type Rejection = ApiError;
    async fn from_request_parts(parts: &mut Parts, app: &AppState) -> Result<Self, Self::Rejection> {
        let auth = Auth::from_request_parts(parts, app).await?;
        if !auth.user.is_admin {
            return Err(ApiError::forbidden("Only Croncave admins can open this."));
        }
        Ok(Admin(auth))
    }
}

#[derive(Deserialize)]
pub struct EmailBody {
    pub email: String,
}

pub fn valid_email(e: &str) -> bool {
    let e = e.trim();
    let Some((local, domain)) = e.split_once('@') else { return false };
    !local.is_empty() && domain.contains('.') && !domain.starts_with('.') && !domain.ends_with('.') && !e.contains(' ')
}

pub async fn send_link(State(app): State<AppState>, Json(b): Json<EmailBody>) -> ApiResult<Json<Value>> {
    let email = b.email.trim().to_lowercase();
    if !valid_email(&email) {
        return Err(ApiError::bad("That doesn't look like an email address."));
    }
    let token = crypto::token();
    sqlx::query("insert into sign_in_tokens (token_hash, email, expires_at) values ($1, $2, $3)")
        .bind(crypto::hash(&token))
        .bind(&email)
        .bind(app.real_now() + Duration::minutes(30))
        .execute(&app.db)
        .await?;
    let link = format!("{}/auth/verify?token={token}", app.cfg.web_url);
    app.providers
        .notifier
        .send(Message {
            channel: Channel::Email,
            to: email.clone(),
            subject: "Your Croncave sign-in link".into(),
            body: format!("Open this link to sign in to Croncave. It works once, for 30 minutes.\n\n{link}"),
            link: Some(link),
        })
        .await?;
    crate::measure::funnel(&app.db, None, "sign_in_link_sent", json!({})).await;
    Ok(Json(json!({ "sent": true })))
}

#[derive(Deserialize)]
pub struct VerifyBody {
    pub token: String,
}

pub async fn verify_link(
    State(app): State<AppState>,
    headers: HeaderMap,
    Json(b): Json<VerifyBody>,
) -> ApiResult<Response> {
    let now = app.real_now();
    let row: Option<(String,)> = sqlx::query_as(
        "update sign_in_tokens set used_at = $2 where token_hash = $1 and used_at is null and expires_at > $2 returning email",
    )
    .bind(crypto::hash(&b.token))
    .bind(now)
    .fetch_optional(&app.db)
    .await?;
    let Some((email,)) = row else {
        return Err(ApiError::bad("This sign-in link has expired or was already used. Ask for a new one."));
    };
    let existing: Option<User> =
        sqlx::query_as("select * from users where email = $1").bind(&email).fetch_optional(&app.db).await?;
    let user_id = match existing {
        Some(u) => u.id,
        None => create_user(&app, &email).await?,
    };
    let token = crypto::token();
    let ua = headers.get("user-agent").and_then(|v| v.to_str().ok()).unwrap_or_default();
    sqlx::query("insert into sessions (token_hash, user_id, user_agent, expires_at) values ($1, $2, $3, $4)")
        .bind(crypto::hash(&token))
        .bind(user_id)
        .bind(ua)
        .bind(now + Duration::days(30))
        .execute(&app.db)
        .await?;
    let cookie = format!("{SESSION_COOKIE}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}", 30 * 86400);
    let mut resp = Json(json!({ "ok": true })).into_response();
    resp.headers_mut().append(SET_COOKIE, HeaderValue::from_str(&cookie).expect("valid cookie"));
    Ok(resp)
}

async fn create_user(app: &AppState, email: &str) -> ApiResult<Uuid> {
    let now = app.now();
    let (version, _) = crate::catalog::current(&app.db, now).await?;
    let user_id = Uuid::new_v4();
    let account_id = Uuid::new_v4();
    let is_admin = app.cfg.admin_emails.iter().any(|a| a == email);
    let name = email.split('@').next().unwrap_or("").split(['.', '_', '+']).next().unwrap_or("");
    let name = name.get(..1).map(|f| f.to_uppercase() + &name[1..]).unwrap_or_default();
    let mut tx = app.db.begin().await?;
    sqlx::query("insert into users (id, email, name, is_admin, prefs) values ($1, $2, $3, $4, $5)")
        .bind(user_id)
        .bind(email)
        .bind(&name)
        .bind(is_admin)
        .bind(
            json!({ "mode": "dark", "scheme": "croncave", "channels": { "email": true, "sms": false, "push": false } }),
        )
        .execute(&mut *tx)
        .await?;
    sqlx::query("insert into accounts (id, name, catalog_version, period_start) values ($1, $2, $3, $4)")
        .bind(account_id)
        .bind(format!("{name}'s account"))
        .bind(version)
        .bind(now)
        .execute(&mut *tx)
        .await?;
    sqlx::query("insert into account_members (account_id, user_id, role) values ($1, $2, 'owner')")
        .bind(account_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    crate::measure::funnel(&app.db, Some(account_id), "signed_up", json!({ "catalog_version": version })).await;
    Ok(user_id)
}

#[derive(Deserialize)]
pub struct PhoneBody {
    pub phone: String,
}

/// US numbers only: ten digits with a valid area code, stored as +1XXXXXXXXXX.
pub fn normalize_us_phone(raw: &str) -> Option<String> {
    let digits: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();
    let ten = match digits.len() {
        10 => digits,
        11 if digits.starts_with('1') => digits[1..].to_string(),
        _ => return None,
    };
    let area = ten.as_bytes()[0];
    let exchange = ten.as_bytes()[3];
    if area == b'0' || area == b'1' || exchange == b'0' || exchange == b'1' {
        return None;
    }
    Some(format!("+1{ten}"))
}

pub async fn send_code(State(app): State<AppState>, auth: Auth, Json(b): Json<PhoneBody>) -> ApiResult<Json<Value>> {
    let phone = normalize_us_phone(&b.phone).ok_or_else(|| {
        ApiError::bad("Enter a US mobile number. Croncave is only available in the United States for now.")
    })?;
    let (recent,): (i64,) = sqlx::query_as("select count(*) from phone_codes where user_id = $1 and expires_at > $2")
        .bind(auth.user.id)
        .bind(app.real_now())
        .fetch_one(&app.db)
        .await?;
    if recent >= 5 {
        return Err(ApiError::bad("Too many codes asked for. Wait a few minutes and try again."));
    }
    let code = crypto::code();
    sqlx::query("insert into phone_codes (id, user_id, phone, code_hash, expires_at) values ($1, $2, $3, $4, $5)")
        .bind(Uuid::new_v4())
        .bind(auth.user.id)
        .bind(&phone)
        .bind(crypto::hash(&code))
        .bind(app.real_now() + Duration::minutes(10))
        .execute(&app.db)
        .await?;
    app.providers
        .notifier
        .send(Message {
            channel: Channel::Sms,
            to: phone.clone(),
            subject: "Croncave code".into(),
            body: format!("Your Croncave code is {code}. It expires in 10 minutes."),
            link: None,
        })
        .await?;
    Ok(Json(json!({ "sent": true, "phone": phone })))
}

#[derive(Deserialize)]
pub struct CodeBody {
    pub code: String,
}

pub async fn verify_code(State(app): State<AppState>, auth: Auth, Json(b): Json<CodeBody>) -> ApiResult<Json<Value>> {
    let row: Option<(Uuid, String, String, i32)> = sqlx::query_as(
        "select id, phone, code_hash, attempts from phone_codes
         where user_id = $1 and used_at is null and expires_at > $2 order by expires_at desc limit 1",
    )
    .bind(auth.user.id)
    .bind(app.real_now())
    .fetch_optional(&app.db)
    .await?;
    let Some((id, phone, hash, attempts)) = row else {
        return Err(ApiError::bad("That code has expired. Ask for a new one."));
    };
    if attempts >= 5 {
        return Err(ApiError::bad("Too many wrong tries. Ask for a new code."));
    }
    if crypto::hash(b.code.trim()) != hash {
        sqlx::query("update phone_codes set attempts = attempts + 1 where id = $1").bind(id).execute(&app.db).await?;
        return Err(ApiError::bad("That code isn't right. Check the text message and try again."));
    }
    let now = app.real_now();
    sqlx::query("update phone_codes set used_at = $2 where id = $1").bind(id).bind(now).execute(&app.db).await?;
    sqlx::query("update users set phone = $2, phone_verified_at = $3 where id = $1")
        .bind(auth.user.id)
        .bind(&phone)
        .bind(now)
        .execute(&app.db)
        .await?;
    crate::measure::funnel(&app.db, Some(auth.account.id), "phone_verified", json!({})).await;
    Ok(Json(json!({ "ok": true })))
}

pub async fn sign_out(State(app): State<AppState>, headers: HeaderMap) -> ApiResult<Response> {
    if let Some(t) = cookie_value(&headers, SESSION_COOKIE) {
        sqlx::query("delete from sessions where token_hash = $1").bind(crypto::hash(&t)).execute(&app.db).await?;
    }
    let mut resp = Json(json!({ "ok": true })).into_response();
    resp.headers_mut()
        .append(SET_COOKIE, HeaderValue::from_static("cc_session=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0"));
    Ok(resp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn us_phones_only() {
        assert_eq!(normalize_us_phone("(415) 555-2671").as_deref(), Some("+14155552671"));
        assert_eq!(normalize_us_phone("+1 415 555 2671").as_deref(), Some("+14155552671"));
        assert_eq!(normalize_us_phone("+44 20 7946 0958"), None);
        assert_eq!(normalize_us_phone("115-555-2671"), None);
        assert_eq!(normalize_us_phone("555-2671"), None);
    }

    #[test]
    fn emails_are_checked_loosely() {
        assert!(valid_email("a@b.co"));
        assert!(!valid_email("a@b"));
        assert!(!valid_email("ab.co"));
        assert!(!valid_email("a b@c.co"));
    }
}
