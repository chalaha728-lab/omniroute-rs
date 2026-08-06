//! Auth middleware — extracts and validates either a JWT (dashboard session)
//! or an API key (sk-or-...) from the Authorization header.
//!
//! Two extractors:
//!   - `DashboardUser` — requires a valid JWT, returns the user claims
//!   - `ApiKeyAuth` — requires a valid API key OR JWT, returns the auth context
//!
//! The /v1/* API accepts either (so the dashboard can call it with its JWT,
//! AND external clients can call it with their sk-or-... key).
//! The /api/dashboard/* routes require a JWT.

use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
};
use axum::RequestExt;
use sqlx::SqlitePool;

use crate::auth;
use crate::error::AppError;
use crate::models::usage::UsageLog;

/// Auth context extracted from a request.
#[derive(Debug, Clone)]
pub struct AuthContext {
    pub user_id: Option<String>,
    pub username: Option<String>,
    pub api_key_id: Option<String>,
    pub source: AuthSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthSource {
    Jwt,
    ApiKey,
}

/// Extractor: requires a valid JWT (dashboard sessions only).
#[derive(Debug, Clone)]
pub struct DashboardUser(pub auth::JwtClaims);

#[axum::async_trait]
impl<S> FromRequestParts<S> for DashboardUser
where
    S: Send + Sync,
    SqlitePool: Clone + Send + Sync + 'static,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let pool = parts.extensions
            .get::<SqlitePool>()
            .cloned()
            .ok_or_else(|| AppError::Internal("DB pool not in extensions".into()))?;
        let config = parts.extensions
            .get::<crate::config::Config>()
            .cloned()
            .ok_or_else(|| AppError::Internal("Config not in extensions".into()))?;

        let token = extract_bearer(parts)
            .ok_or_else(|| AppError::Unauthorized("missing Authorization header".into()))?;

        let claims = auth::verify_jwt(&token, &config.jwt_secret)?;
        // Verify the user still exists in the DB
        let exists: Option<(String,)> = sqlx::query_as("SELECT id FROM users WHERE id = ?")
            .bind(&claims.sub)
            .fetch_optional(&pool)
            .await?;
        if exists.is_none() {
            return Err(AppError::Unauthorized("user not found".into()));
        }
        Ok(DashboardUser(claims))
    }
}

/// Extractor: accepts either a JWT or an API key (for /v1/* API).
#[derive(Debug, Clone)]
pub struct ApiKeyAuth(pub AuthContext);

#[axum::async_trait]
impl<S> FromRequestParts<S> for ApiKeyAuth
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let pool = parts.extensions
            .get::<SqlitePool>()
            .cloned()
            .ok_or_else(|| AppError::Internal("DB pool not in extensions".into()))?;
        let config = parts.extensions
            .get::<crate::config::Config>()
            .cloned()
            .ok_or_else(|| AppError::Internal("Config not in extensions".into()))?;

        let token = extract_bearer(parts)
            .ok_or_else(|| AppError::Unauthorized("missing Authorization header".into()))?;

        // Try JWT first
        if let Ok(claims) = auth::verify_jwt(&token, &config.jwt_secret) {
            return Ok(ApiKeyAuth(AuthContext {
                user_id: Some(claims.sub.clone()),
                username: Some(claims.username.clone()),
                api_key_id: None,
                source: AuthSource::Jwt,
            }));
        }

        // Fall back to API key lookup
        let key_hash = auth::hash_api_key(&token);
        let row: Option<(String, String, i64)> = sqlx::query_as(
            "SELECT id, user_id, enabled FROM api_keys WHERE key_hash = ? AND expires_at IS NULL OR expires_at > datetime('now')"
        )
        .bind(&key_hash)
        .fetch_optional(&pool)
        .await
        .map_err(|e| AppError::Internal(format!("api key lookup failed: {}", e)))?;

        match row {
            Some((id, user_id, enabled)) if enabled == 1 => {
                // Update last_used_at
                let _ = sqlx::query("UPDATE api_keys SET last_used_at = datetime('now') WHERE id = ?")
                    .bind(&id)
                    .execute(&pool)
                    .await;
                Ok(ApiKeyAuth(AuthContext {
                    user_id: Some(user_id),
                    username: None,
                    api_key_id: Some(id),
                    source: AuthSource::ApiKey,
                }))
            }
            _ => Err(AppError::Unauthorized("invalid API key".into())),
        }
    }
}

fn extract_bearer(parts: &Parts) -> Option<String> {
    let auth_header = parts.headers.get("authorization")?.to_str().ok()?;
    if let Some(token) = auth_header.strip_prefix("Bearer ") {
        return Some(token.trim().to_string());
    }
    if let Some(token) = auth_header.strip_prefix("bearer ") {
        return Some(token.trim().to_string());
    }
    Some(auth_header.trim().to_string())
}

/// Record a usage log entry. Best-effort — errors are logged but not surfaced.
pub async fn record_usage(pool: &SqlitePool, log: &UsageLog) {
    let _ = sqlx::query(
        r#"INSERT INTO usage_logs
            (id, api_key_id, user_id, provider_id, model, endpoint, method, status_code,
             prompt_tokens, completion_tokens, total_tokens, duration_ms, error, client_ip, user_agent)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&log.id)
    .bind(&log.api_key_id)
    .bind(&log.user_id)
    .bind(&log.provider_id)
    .bind(&log.model)
    .bind(&log.endpoint)
    .bind(&log.method)
    .bind(log.status_code)
    .bind(log.prompt_tokens)
    .bind(log.completion_tokens)
    .bind(log.total_tokens)
    .bind(log.duration_ms)
    .bind(&log.error)
    .bind(&log.client_ip)
    .bind(&log.user_agent)
    .execute(pool)
    .await
    .map_err(|e| tracing::warn!("[usage] failed to record: {}", e));
}
