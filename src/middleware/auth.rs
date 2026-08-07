//! Auth middleware — extracts and validates either a JWT (dashboard session)
//! or an API key (sk-or-...) from the Authorization header.

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use sqlx::SqlitePool;

use crate::auth;
use crate::config::Config;
use crate::error::AppError;
use crate::models::usage::UsageLog;

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

#[derive(Debug, Clone)]
pub struct DashboardUser(pub auth::JwtClaims);

#[axum::async_trait]
impl<S> FromRequestParts<S> for DashboardUser
where
    S: Send + Sync,
    SqlitePool: axum::extract::FromRef<S>,
    Config: axum::extract::FromRef<S>,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let pool = <SqlitePool as axum::extract::FromRef<S>>::from_ref(state);
        let config = <Config as axum::extract::FromRef<S>>::from_ref(state);

        let token = extract_bearer(parts)
            .ok_or_else(|| AppError::Unauthorized("missing Authorization header".into()))?;

        let claims = auth::verify_jwt(&token, &config.jwt_secret)?;
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

#[derive(Debug, Clone)]
pub struct ApiKeyAuth(pub AuthContext);

#[axum::async_trait]
impl<S> FromRequestParts<S> for ApiKeyAuth
where
    S: Send + Sync,
    SqlitePool: axum::extract::FromRef<S>,
    Config: axum::extract::FromRef<S>,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let pool = <SqlitePool as axum::extract::FromRef<S>>::from_ref(state);
        let config = <Config as axum::extract::FromRef<S>>::from_ref(state);

        let token = extract_bearer(parts)
            .ok_or_else(|| AppError::Unauthorized("missing Authorization header".into()))?;

        if let Ok(claims) = auth::verify_jwt(&token, &config.jwt_secret) {
            return Ok(ApiKeyAuth(AuthContext {
                user_id: Some(claims.sub.clone()),
                username: Some(claims.username.clone()),
                api_key_id: None,
                source: AuthSource::Jwt,
            }));
        }

        let key_hash = auth::hash_api_key(&token);
        let row: Option<(String, String, i64)> = sqlx::query_as(
            "SELECT id, user_id, enabled FROM api_keys WHERE key_hash = ? AND (expires_at IS NULL OR expires_at > datetime(\'now\'))"
        )
        .bind(&key_hash)
        .fetch_optional(&pool)
        .await
        .map_err(|e| AppError::Internal(format!("api key lookup failed: {}", e)))?;

        match row {
            Some((id, user_id, enabled)) if enabled == 1 => {
                let _ = sqlx::query("UPDATE api_keys SET last_used_at = datetime(\'now\') WHERE id = ?")
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
