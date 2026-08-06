//! Per-org quota enforcement — checks + increments org-level token quotas.
//!
//! Mirrors the per-key QuotaPlugin but at the organization level.
//! Tables: org_quotas (created by migration 003).

use sqlx::SqlitePool;
use serde::{Deserialize, Serialize};

use crate::error::AppError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrgQuota {
    pub org_id: String,
    pub limit_tokens: u64,
    pub used_tokens: u64,
    pub window_days: u32,
    pub reset_at: String,
}

/// Check if the org has exceeded its quota. Returns Err(429) if so.
pub async fn check(pool: &SqlitePool, org_id: Option<&str>) -> Result<(), AppError> {
    let org_id = match org_id {
        Some(id) => id,
        None => return Ok(()), // personal key — no org quota
    };

    let row: Option<(i64, i64, String, i64)> = sqlx::query_as(
        "SELECT limit_tokens, used_tokens, reset_at, window_days FROM org_quotas WHERE org_id = ?"
    )
    .bind(org_id)
    .fetch_optional(pool)
    .await?;

    let (limit, used, reset_at, window_days) = match row {
        Some(r) => r,
        None => return Ok(()), // no quota configured = unlimited
    };

    let now = chrono::Utc::now();
    let reset = chrono::DateTime::parse_from_rfc3339(&reset_at)
        .map(|d| d.with_timezone(&chrono::Utc))
        .unwrap_or(now);

    if now > reset {
        // Window rolled — allow the request; on_usage will reset.
        return Ok(());
    }

    if used as u64 >= limit as u64 {
        return Err(AppError::RateLimited(format!(
            "org quota exceeded: used {} of {} tokens (resets {})",
            used, limit, reset_at
        )));
    }

    let _ = window_days; // suppress unused warning
    Ok(())
}

/// Increment the org's used token counter.
pub async fn increment(pool: &SqlitePool, org_id: Option<&str>, tokens: u32) {
    let org_id = match org_id {
        Some(id) => id,
        None => return,
    };

    let row: Option<(i64, i64, String, i64)> = sqlx::query_as(
        "SELECT limit_tokens, used_tokens, reset_at, window_days FROM org_quotas WHERE org_id = ?"
    )
    .bind(org_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    let (limit, mut used, reset_at, window_days) = match row {
        Some(r) => r,
        None => return, // no quota = don't track
    };

    let now = chrono::Utc::now();
    let reset = chrono::DateTime::parse_from_rfc3339(&reset_at)
        .map(|d| d.with_timezone(&chrono::Utc))
        .unwrap_or(now);

    if now > reset {
        // Window rolled — reset used to 0
        used = 0;
        let new_reset = (now + chrono::Duration::days(window_days as i64)).to_rfc3339();
        let _ = sqlx::query(
            "UPDATE org_quotas SET used_tokens = ?, reset_at = ?, updated_at = datetime('now') WHERE org_id = ?"
        )
        .bind(tokens as i64)
        .bind(&new_reset)
        .bind(org_id)
        .execute(pool)
        .await;
    } else {
        used = used.saturating_add(tokens as i64);
        let _ = sqlx::query(
            "UPDATE org_quotas SET used_tokens = ?, updated_at = datetime('now') WHERE org_id = ?"
        )
        .bind(used)
        .bind(org_id)
        .execute(pool)
        .await;
    }

    let _ = limit; // suppress unused warning
}

/// Set/update the quota for an org.
pub async fn set(
    pool: &SqlitePool,
    org_id: &str,
    limit_tokens: u64,
    window_days: u32,
) -> Result<OrgQuota, AppError> {
    let reset_at = (chrono::Utc::now() + chrono::Duration::days(window_days as i64)).to_rfc3339();
    sqlx::query(
        r#"INSERT INTO org_quotas (org_id, limit_tokens, used_tokens, window_days, reset_at)
           VALUES (?, ?, 0, ?, ?)
           ON CONFLICT(org_id) DO UPDATE SET
             limit_tokens = excluded.limit_tokens,
             window_days = excluded.window_days,
             reset_at = excluded.reset_at,
             updated_at = datetime('now')"#,
    )
    .bind(org_id)
    .bind(limit_tokens as i64)
    .bind(window_days as i64)
    .bind(&reset_at)
    .execute(pool)
    .await?;

    Ok(OrgQuota {
        org_id: org_id.into(),
        limit_tokens,
        used_tokens: 0,
        window_days,
        reset_at,
    })
}

/// Get the current quota for an org.
pub async fn get(pool: &SqlitePool, org_id: &str) -> Option<OrgQuota> {
    let row: Option<(i64, i64, i64, String)> = sqlx::query_as(
        "SELECT limit_tokens, used_tokens, window_days, reset_at FROM org_quotas WHERE org_id = ?"
    )
    .bind(org_id)
    .fetch_optional(pool)
    .await
    .ok()?;

    let (limit, used, window, reset_at) = row?;
    Some(OrgQuota {
        org_id: org_id.into(),
        limit_tokens: limit as u64,
        used_tokens: used as u64,
        window_days: window as u32,
        reset_at,
    })
}
