//! API errors carry a plain-words message for the person.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub message: String,
    /// A machine-readable reason the web app can act on (e.g. `needs_sign_in`).
    pub code: Option<&'static str>,
}

pub type ApiResult<T> = Result<T, ApiError>;

impl ApiError {
    pub fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self { status, message: message.into(), code: None }
    }
    pub fn bad(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, message)
    }
    pub fn not_found(what: &str) -> Self {
        Self::new(StatusCode::NOT_FOUND, format!("{what} wasn't found."))
    }
    pub fn unauthorized() -> Self {
        Self { status: StatusCode::UNAUTHORIZED, message: "Please sign in.".into(), code: Some("needs_sign_in") }
    }
    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, message)
    }
    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, message)
    }
    /// A plan limit stopped this; the web app offers the Plans page.
    pub fn limit(message: impl Into<String>) -> Self {
        Self { status: StatusCode::PAYMENT_REQUIRED, message: message.into(), code: Some("plan_limit") }
    }
    pub fn unavailable(message: impl Into<String>) -> Self {
        Self::new(StatusCode::SERVICE_UNAVAILABLE, message)
    }
    pub fn with_code(mut self, code: &'static str) -> Self {
        self.code = Some(code);
        self
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "error": self.message, "code": self.code }))).into_response()
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        tracing::error!(error = %e, "database error");
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong on our side. Please try again.")
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        tracing::error!(error = %e, "internal error");
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong on our side. Please try again.")
    }
}

impl From<croncave_relay::RelayError> for ApiError {
    fn from(e: croncave_relay::RelayError) -> Self {
        use croncave_relay::RelayError::*;
        match e {
            Reset(msg) => Self::bad(msg),
            NotConnected => Self::unavailable("Your computer isn't awake yet. Try again in a moment."),
            other => Self::unavailable(format!("Couldn't reach your computer: {other}")),
        }
    }
}
