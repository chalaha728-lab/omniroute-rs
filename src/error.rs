//! Unified error type — converts to HTTP responses with the OpenAI error shape.
//!
//! OpenAI error format (consumed by Cursor, Cline, Codex, etc.):
//!   { "error": { "message": "...", "type": "...", "code": "...", "param": null } }

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("unauthorized: {0}")]
    Unauthorized(String),

    #[error("forbidden: {0}")]
    Forbidden(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("bad request: {0}")]
    BadRequest(String),

    #[error("rate limited: {0}")]
    RateLimited(String),

    #[error("provider error: {0}")]
    Provider(String),

    #[error("all providers failed")]
    AllProvidersFailed,

    #[error("internal error: {0}")]
    Internal(String),

    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("jwt error: {0}")]
    Jwt(#[from] jsonwebtoken::errors::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        AppError::Internal(e.to_string())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_type, message) = match &self {
            AppError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, "authentication_error", msg.clone()),
            AppError::Forbidden(msg) => (StatusCode::FORBIDDEN, "authorization_error", msg.clone()),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, "not_found_error", msg.clone()),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, "invalid_request_error", msg.clone()),
            AppError::RateLimited(msg) => (StatusCode::TOO_MANY_REQUESTS, "rate_limit_error", msg.clone()),
            AppError::Provider(msg) => (StatusCode::BAD_GATEWAY, "provider_error", msg.clone()),
            AppError::AllProvidersFailed => (
                StatusCode::BAD_GATEWAY,
                "provider_error",
                "All configured providers failed or are unavailable".into(),
            ),
            AppError::Database(e) => {
                tracing::error!("database error: {:?}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "server_error", "internal database error".into())
            }
            AppError::Jwt(_) => (StatusCode::UNAUTHORIZED, "authentication_error", "invalid token".into()),
            AppError::Io(e) => {
                tracing::error!("io error: {:?}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "server_error", "internal io error".into())
            }
            AppError::Internal(msg) => {
                tracing::error!("internal error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, "server_error", msg.clone())
            }
        };

        let body = Json(json!({
            "error": {
                "message": message,
                "type": error_type,
                "code": null,
                "param": null,
            }
        }));

        (status, body).into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;
