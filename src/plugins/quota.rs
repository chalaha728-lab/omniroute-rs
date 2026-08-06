//! Quota plugin — enforces per-API-key token quotas.
//!
//! Quotas are stored in the `settings` table as JSON:
//!   key = "quota:<api_key_id>"
//!   value = { "limit": 1000000, "used": 453201, "window_days": 30, "reset_at": "2026-09-06T..." }
//!
//! When `used >= limit`, the request is rejected with 429 Too Many Requests.
//! Quotas are configured via the dashboard API (PUT /api/dashboard/api-keys/:id/quota).
//!
//! Opt-in via env var: OMNIROUTE_QUOTA_ENABLED=true

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::sync::Arc;

use crate::error::AppError;
use crate::models::chat::ChatCompletionRequest;
use crate::models::usage::UsageLog;
use crate::plugins::Plugin;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quota {
    pub limit: u64,           // max tokens in the window
    pub used: u64,            // tokens used so far
    pub window_days: u32,     // rolling window (default 30)
    pub reset_at: String,     // ISO 8601 — when the window rolls over
}

impl Default for Quota {
    fn default() -> Self {
        Self {
            limit: 1_000_000,
            used: 0,
            window_days: 30,
            reset_at: chrono::Utc::now().to_rfc3339(),
        }
    }
}

pub struct QuotaPlugin {
    pool: Arc<SqlitePool>,
}

impl QuotaPlugin {
    pub fn new(pool: Arc<SqlitePool>) -> Self {
        Self { pool }
    }

    fn is_enabled() -> bool {
        std::env::var("OMNIROUTE_QUOTA_ENABLED")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false)
    }

    async fn get_quota(&self, api_key_id: &str) -> Option<Quota> {
        let key = format!("quota:{}", api_key_id);
        let row: Option<(String,)> = sqlx::query_as("SELECT value FROM settings WHERE key = ?")
            .bind(&key)
            .fetch_optional(&*self.pool)
            .await
            .ok()?;
        let (value,) = row?;
        serde_json::from_str(&value).ok()
    }

    async fn set_quota(&self, api_key_id: &str, quota: &Quota) -> Result<(), AppError> {
        let key = format!("quota:{}", api_key_id);
        let value = serde_json::to_string(quota)
            .map_err(|e| AppError::Internal(format!("quota serialize failed: {}", e)))?;
        sqlx::query(
            "INSERT INTO settings (key, value, updated_at) VALUES (?, ?, datetime('now')) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')"
        )
        .bind(&key)
        .bind(&value)
        .execute(&*self.pool)
        .await
        .map_err(|e| AppError::Database(e))?;
        Ok(())
    }

    /// Check if the API key has exceeded its quota. Returns Err(429) if so.
    async fn check_quota(&self, api_key_id: Option<&str>) -> Result<(), AppError> {
        if !Self::is_enabled() {
            return Ok(());
        }
        let api_key_id = match api_key_id {
            Some(id) => id,
            None => return Ok(()), // JWT auth — no quota
        };

        let quota = match self.get_quota(api_key_id).await {
            Some(q) => q,
            None => return Ok(()), // no quota configured = unlimited
        };

        // Check window reset
        let now = chrono::Utc::now();
        let reset_at = chrono::DateTime::parse_from_rfc3339(&quota.reset_at)
            .map(|d| d.with_timezone(&chrono::Utc))
            .unwrap_or(now);
        if now > reset_at {
            // Window has rolled — would reset used to 0 here. For simplicity,
            // we just allow the request; the next on_usage will reset it.
            return Ok(());
        }

        if quota.used >= quota.limit {
            return Err(AppError::RateLimited(format!(
                "token quota exceeded: used {} of {} (resets {})",
                quota.used, quota.limit, quota.reset_at
            )));
        }

        Ok(())
    }

    /// Increment the used counter after a request completes.
    async fn increment_usage(&self, api_key_id: Option<&str>, tokens: u32) {
        if !Self::is_enabled() {
            return;
        }
        let api_key_id = match api_key_id {
            Some(id) => id,
            None => return,
        };

        let mut quota = match self.get_quota(api_key_id).await {
            Some(q) => q,
            None => return, // no quota = don't track
        };

        // Check window reset
        let now = chrono::Utc::now();
        let reset_at = chrono::DateTime::parse_from_rfc3339(&quota.reset_at)
            .map(|d| d.with_timezone(&chrono::Utc))
            .unwrap_or(now);
        if now > reset_at {
            // Reset window
            quota.used = 0;
            quota.reset_at = (now + chrono::Duration::days(quota.window_days as i64)).to_rfc3339();
        }

        quota.used = quota.used.saturating_add(tokens as u64);
        let _ = self.set_quota(api_key_id, &quota).await;
    }
}

#[async_trait]
impl Plugin for QuotaPlugin {
    fn name(&self) -> &'static str { "quota" }
    fn priority(&self) -> i32 { 10 } // after logging, before request transforms

    async fn before_request(&self, req: &mut ChatCompletionRequest) -> Result<(), AppError> {
        // We need the api_key_id from the auth context — but Plugin::before_request
        // doesn't receive it. In practice, the chat route checks the quota directly
        // before calling the provider; this hook is a no-op for now.
        // The actual check happens in routes::chat::chat_completions via
        // QuotaPlugin::check_quota_from_request().
        let _ = req;
        Ok(())
    }

    async fn on_usage(&self, log: &UsageLog) {
        if let Some(api_key_id) = &log.api_key_id {
            self.increment_usage(Some(api_key_id), log.total_tokens as u32).await;
        }
    }
}

/// Standalone helper — called from the chat route with the auth context.
pub async fn check_quota_for_api_key(pool: &SqlitePool, api_key_id: Option<&str>) -> Result<(), AppError> {
    if !QuotaPlugin::is_enabled() {
        return Ok(());
    }
    let plugin = QuotaPlugin::new(Arc::new(pool.clone()));
    plugin.check_quota(api_key_id).await
}

/// Set/update the quota for an API key. Called from the dashboard route.
pub async fn set_quota_for_api_key(
    pool: &SqlitePool,
    api_key_id: &str,
    limit: u64,
    window_days: u32,
) -> Result<Quota, AppError> {
    let quota = Quota {
        limit,
        used: 0,
        window_days,
        reset_at: (chrono::Utc::now() + chrono::Duration::days(window_days as i64)).to_rfc3339(),
    };
    let plugin = QuotaPlugin::new(Arc::new(pool.clone()));
    plugin.set_quota(api_key_id, &quota).await?;
    Ok(quota)
}

/// Get the current quota + usage for an API key.
pub async fn get_quota_for_api_key(pool: &SqlitePool, api_key_id: &str) -> Option<Quota> {
    let plugin = QuotaPlugin::new(Arc::new(pool.clone()));
    plugin.get_quota(api_key_id).await
}
