//! /api/auth/* — login, verify, change password.

use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::SqlitePool;

use crate::auth;
use crate::config::Config;
use crate::error::{AppError, AppResult};
use crate::middleware::auth::DashboardUser;

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub username: String,
    pub role: String,
}

pub async fn login(
    State(pool): State<SqlitePool>,
    State(config): State<Config>,
    Json(req): Json<LoginRequest>,
) -> AppResult<Json<Value>> {
    let row: Option<(String, String, String)> = sqlx::query_as(
        "SELECT id, password_hash, role FROM users WHERE username = ?"
    )
    .bind(&req.username)
    .fetch_optional(&pool)
    .await?;

    let (user_id, password_hash, role) = row.ok_or_else(|| {
        AppError::Unauthorized("invalid username or password".into())
    })?;

    if !auth::verify_password(&req.password, &password_hash) {
        return Err(AppError::Unauthorized("invalid username or password".into()));
    }

    let token = auth::issue_jwt(&user_id, &req.username, &role, &config.jwt_secret)?;
    Ok(Json(json!({
        "token": token,
        "user": {
            "id": user_id,
            "username": req.username,
            "role": role,
        }
    })))
}

pub async fn verify(user: DashboardUser) -> AppResult<Json<Value>> {
    let claims = user.0;
    Ok(Json(json!({
        "valid": true,
        "user": {
            "id": claims.sub,
            "username": claims.username,
            "role": claims.role,
        }
    })))
}

#[derive(Debug, Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

pub async fn change_password(
    State(pool): State<SqlitePool>,
    user: DashboardUser,
    Json(req): Json<ChangePasswordRequest>,
) -> AppResult<Json<Value>> {
    if req.new_password.len() < 8 {
        return Err(AppError::BadRequest("password must be at least 8 chars".into()));
    }
    let claims = user.0;

    let row: Option<(String,)> = sqlx::query_as("SELECT password_hash FROM users WHERE id = ?")
        .bind(&claims.sub)
        .fetch_optional(&pool)
        .await?;
    let (current_hash,) = row.ok_or_else(|| AppError::NotFound("user not found".into()))?;

    if !auth::verify_password(&req.current_password, &current_hash) {
        return Err(AppError::Unauthorized("current password is incorrect".into()));
    }

    let new_hash = auth::hash_password(&req.new_password)
        .map_err(|e| AppError::Internal(format!("hash failed: {}", e)))?;
    sqlx::query("UPDATE users SET password_hash = ?, updated_at = datetime('now') WHERE id = ?")
        .bind(&new_hash)
        .bind(&claims.sub)
        .execute(&pool)
        .await?;
    Ok(Json(json!({ "success": true })))
}
