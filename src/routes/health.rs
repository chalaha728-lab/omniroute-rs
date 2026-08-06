//! /api/monitoring/health — liveness + readiness probe.

use axum::extract::State;
use axum::Json;
use serde_json::{json, Value};
use sqlx::SqlitePool;

use crate::error::AppResult;

pub async fn health(State(pool): State<SqlitePool>) -> AppResult<Json<Value>> {
    let db_ok = crate::db::ping(&pool).await;
    Ok(Json(json!({
        "status": if db_ok { "ok" } else { "degraded" },
        "db": db_ok,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    })))
}
