//! /api/dashboard/* — provider config, API key management, usage stats.

use axum::extract::{Path, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::SqlitePool;

use crate::auth;
use crate::config::Config;
use crate::error::{AppError, AppResult};
use crate::middleware::auth::{ApiKeyAuth, AuthContext, DashboardUser};
use crate::models::provider::ProviderId;
use crate::models::usage::{ProviderUsage, UsageSummary};

// ─── GET /api/dashboard/usage ───────────────────────────────────────────────

pub async fn usage_summary(
    State(pool): State<SqlitePool>,
    _user: DashboardUser,
) -> AppResult<Json<Value>> {
    let total_requests: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM usage_logs")
        .fetch_one(&pool).await.unwrap_or(0);
    let total_tokens: i64 = sqlx::query_scalar("SELECT COALESCE(SUM(total_tokens), 0) FROM usage_logs")
        .fetch_one(&pool).await.unwrap_or(0);
    let total_prompt: i64 = sqlx::query_scalar("SELECT COALESCE(SUM(prompt_tokens), 0) FROM usage_logs")
        .fetch_one(&pool).await.unwrap_or(0);
    let total_completion: i64 = sqlx::query_scalar("SELECT COALESCE(SUM(completion_tokens), 0) FROM usage_logs")
        .fetch_one(&pool).await.unwrap_or(0);
    let error_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM usage_logs WHERE status_code >= 400")
        .fetch_one(&pool).await.unwrap_or(0);

    let by_provider_rows: Vec<(String, i64, i64, i64)> = sqlx::query_as(
        "SELECT provider_id, COUNT(*), COALESCE(SUM(total_tokens), 0), SUM(CASE WHEN status_code >= 400 THEN 1 ELSE 0 END) FROM usage_logs GROUP BY provider_id"
    ).fetch_all(&pool).await.unwrap_or_default();

    let by_provider: Vec<ProviderUsage> = by_provider_rows.into_iter()
        .map(|(pid, reqs, tokens, errs)| ProviderUsage {
            provider_id: pid, requests: reqs, tokens, errors: errs,
        }).collect();

    let summary = UsageSummary {
        total_requests, total_tokens,
        total_prompt_tokens: total_prompt,
        total_completion_tokens: total_completion,
        error_count, by_provider,
    };
    Ok(Json(json!(summary)))
}

// ─── GET /api/dashboard/providers ───────────────────────────────────────────

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ProviderRow {
    pub id: String,
    pub display_name: String,
    pub base_url: Option<String>,
    pub enabled: bool,
    pub priority: i64,
    pub has_key: bool,
}

pub async fn list_providers(
    State(pool): State<SqlitePool>,
    _user: DashboardUser,
) -> AppResult<Json<Value>> {
    let rows: Vec<(String, String, Option<String>, i64, i64, Option<String>)> = sqlx::query_as(
        "SELECT id, display_name, base_url, enabled, priority, api_key_enc FROM providers ORDER BY priority"
    ).fetch_all(&pool).await?;

    let providers: Vec<ProviderRow> = rows.into_iter().map(|(id, display_name, base_url, enabled, priority, key_enc)| {
        ProviderRow {
            id, display_name, base_url, enabled: enabled != 0, priority,
            has_key: key_enc.is_some(),
        }
    }).collect();
    Ok(Json(json!(providers)))
}

// ─── PUT /api/dashboard/providers/:id ───────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct UpdateProviderRequest {
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub enabled: Option<bool>,
}

pub async fn update_provider(
    State(pool): State<SqlitePool>,
    State(config): State<Config>,
    _user: DashboardUser,
    Path(id): Path<String>,
    Json(req): Json<UpdateProviderRequest>,
) -> AppResult<Json<Value>> {
    // Validate provider id
    if ProviderId::from_str(&id).is_none() {
        return Err(AppError::BadRequest(format!("unknown provider: {}", id)));
    }

    if let Some(key) = &req.api_key {
        let enc = auth::encrypt_api_key(key, &config.api_key_secret);
        sqlx::query("UPDATE providers SET api_key_enc = ?, updated_at = datetime('now') WHERE id = ?")
            .bind(&enc).bind(&id).execute(&pool).await?;
    }
    if let Some(url) = &req.base_url {
        sqlx::query("UPDATE providers SET base_url = ?, updated_at = datetime('now') WHERE id = ?")
            .bind(url).bind(&id).execute(&pool).await?;
    }
    if let Some(enabled) = req.enabled {
        sqlx::query("UPDATE providers SET enabled = ?, updated_at = datetime('now') WHERE id = ?")
            .bind(if enabled { 1 } else { 0 }).bind(&id).execute(&pool).await?;
    }
    Ok(Json(json!({ "success": true })))
}

// ─── GET /api/dashboard/api-keys ────────────────────────────────────────────

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ApiKeyRow {
    pub id: String,
    pub name: String,
    pub prefix: String,
    pub created_at: String,
    pub last_used_at: Option<String>,
    pub expires_at: Option<String>,
    pub enabled: bool,
}

pub async fn list_api_keys(
    State(pool): State<SqlitePool>,
    user: DashboardUser,
) -> AppResult<Json<Value>> {
    let rows: Vec<(String, String, String, String, Option<String>, Option<String>, i64)> = sqlx::query_as(
        "SELECT id, name, prefix, created_at, last_used_at, expires_at, enabled FROM api_keys WHERE user_id = ? ORDER BY created_at DESC"
    ).bind(&user.0.sub).fetch_all(&pool).await?;

    let keys: Vec<ApiKeyRow> = rows.into_iter().map(|(id, name, prefix, created_at, last_used_at, expires_at, enabled)| {
        ApiKeyRow { id, name, prefix, created_at, last_used_at, expires_at, enabled: enabled != 0 }
    }).collect();
    Ok(Json(json!(keys)))
}

// ─── POST /api/dashboard/api-keys ───────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateApiKeyRequest {
    pub name: String,
    pub expires_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CreateApiKeyResponse {
    pub id: String,
    pub key: String,       // returned ONCE — the full plaintext key
    pub prefix: String,
    pub name: String,
}

pub async fn create_api_key(
    State(pool): State<SqlitePool>,
    State(config): State<Config>,
    user: DashboardUser,
    Json(req): Json<CreateApiKeyRequest>,
) -> AppResult<Json<Value>> {
    let plaintext = auth::generate_api_key();
    let key_hash = auth::hash_api_key(&plaintext);
    let key_enc = auth::encrypt_api_key(&plaintext, &config.api_key_secret);
    let prefix = plaintext.chars().take(12).collect::<String>();
    let id = uuid::Uuid::new_v4().to_string();

    sqlx::query(
        "INSERT INTO api_keys (id, name, key_hash, key_encrypted, prefix, user_id, expires_at) VALUES (?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(&id).bind(&req.name).bind(&key_hash).bind(&key_enc).bind(&prefix)
    .bind(&user.0.sub).bind(&req.expires_at).execute(&pool).await?;

    Ok(Json(json!({
        "id": id,
        "key": plaintext,
        "prefix": prefix,
        "name": req.name,
    })))
}

// ─── DELETE /api/dashboard/api-keys/:id ─────────────────────────────────────

pub async fn delete_api_key(
    State(pool): State<SqlitePool>,
    _user: DashboardUser,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    sqlx::query("DELETE FROM api_keys WHERE id = ?")
        .bind(&id).execute(&pool).await?;
    Ok(Json(json!({ "success": true })))
}

// ─── GET /api/dashboard/whoami ──────────────────────────────────────────────
//
// Returns the auth context for the current request — useful for the dashboard
// to know if the request was authenticated via JWT or API key.

pub async fn whoami(auth: ApiKeyAuth) -> AppResult<Json<Value>> {
    let ctx: AuthContext = auth.0;
    Ok(Json(json!({
        "user_id": ctx.user_id,
        "username": ctx.username,
        "api_key_id": ctx.api_key_id,
        "source": match ctx.source {
            crate::middleware::auth::AuthSource::Jwt => "jwt",
            crate::middleware::auth::AuthSource::ApiKey => "api_key",
        },
    })))
}
