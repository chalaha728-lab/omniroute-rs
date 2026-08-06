//! Multi-tenant support — organizations + per-org API keys + per-org usage.
//!
//! Routes:
//!   POST   /api/dashboard/orgs                       — create org
//!   GET    /api/dashboard/orgs                       — list user's orgs
//!   GET    /api/dashboard/orgs/:id                   — get org details
//!   PUT    /api/dashboard/orgs/:id                   — update org (name, plan)
//!   DELETE /api/dashboard/orgs/:id                   — delete org (owner only)
//!   POST   /api/dashboard/orgs/:id/members           — add member
//!   DELETE /api/dashboard/orgs/:id/members/:userId   — remove member
//!   GET    /api/dashboard/orgs/:id/usage             — org usage stats
//!   GET    /api/dashboard/orgs/:id/quota             — org quota status
//!   PUT    /api/dashboard/orgs/:id/quota             — set org quota (admin)

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use axum::extract::{Path, State};
use axum::Json;
use serde_json::{json, Value};

use crate::error::{AppError, AppResult};
use crate::middleware::auth::DashboardUser;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Organization {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub owner_user_id: String,
    pub plan: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct OrgMember {
    pub org_id: String,
    pub user_id: String,
    pub role: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateOrgRequest {
    pub name: String,
    pub slug: Option<String>,
    pub plan: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateOrgRequest {
    pub name: Option<String>,
    pub plan: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AddMemberRequest {
    pub user_id: String,
    pub role: Option<String>,
}

/// Generate a slug from a name (lowercase, hyphenated, alphanumeric only).
fn slugify(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

/// Create a new organization.
pub async fn create_org(
    State(pool): State<SqlitePool>,
    user: DashboardUser,
    Json(req): Json<CreateOrgRequest>,
) -> AppResult<Json<Value>> {
    if req.name.is_empty() {
        return Err(AppError::BadRequest("name is required".into()));
    }
    let slug = req.slug.unwrap_or_else(|| slugify(&req.name));
    let plan = req.plan.unwrap_or_else(|| "free".into());
    let id = uuid::Uuid::new_v4().to_string();

    sqlx::query(
        r#"INSERT INTO organizations (id, name, slug, owner_user_id, plan)
           VALUES (?, ?, ?, ?, ?)"#,
    )
    .bind(&id)
    .bind(&req.name)
    .bind(&slug)
    .bind(&user.0.sub)
    .bind(&plan)
    .execute(&pool)
    .await?;

    // Owner is automatically a member with role 'owner'
    sqlx::query(
        "INSERT INTO organization_members (org_id, user_id, role) VALUES (?, ?, 'owner')",
    )
    .bind(&id)
    .bind(&user.0.sub)
    .execute(&pool)
    .await?;

    Ok(Json(json!({
        "id": id,
        "name": req.name,
        "slug": slug,
        "plan": plan,
        "owner_user_id": user.0.sub,
    })))
}

/// List orgs the current user belongs to.
pub async fn list_orgs(
    State(pool): State<SqlitePool>,
    user: DashboardUser,
) -> AppResult<Json<Value>> {
    let orgs: Vec<(String, String, String, String, String, String, String)> = sqlx::query_as(
        r#"SELECT o.id, o.name, o.slug, o.owner_user_id, o.plan, o.created_at, o.updated_at
           FROM organizations o
           JOIN organization_members m ON m.org_id = o.id
           WHERE m.user_id = ?
           ORDER BY o.created_at DESC"#,
    )
    .bind(&user.0.sub)
    .fetch_all(&pool)
    .await?;

    let result: Vec<Organization> = orgs.into_iter()
        .map(|(id, name, slug, owner_user_id, plan, created_at, updated_at)| Organization {
            id, name, slug, owner_user_id, plan, created_at, updated_at,
        })
        .collect();

    Ok(Json(json!(result)))
}

/// Get an org by id (must be a member).
pub async fn get_org(
    State(pool): State<SqlitePool>,
    user: DashboardUser,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    let row: Option<(String, String, String, String, String, String, String)> = sqlx::query_as(
        r#"SELECT o.id, o.name, o.slug, o.owner_user_id, o.plan, o.created_at, o.updated_at
           FROM organizations o
           JOIN organization_members m ON m.org_id = o.id
           WHERE o.id = ? AND m.user_id = ?"#,
    )
    .bind(&id)
    .bind(&user.0.sub)
    .fetch_optional(&pool)
    .await?;

    let row = row.ok_or_else(|| AppError::NotFound("org not found or not a member".into()))?;
    let org = Organization {
        id: row.0, name: row.1, slug: row.2, owner_user_id: row.3,
        plan: row.4, created_at: row.5, updated_at: row.6,
    };
    Ok(Json(json!(org)))
}

/// Update an org (owner or admin only).
pub async fn update_org(
    State(pool): State<SqlitePool>,
    user: DashboardUser,
    Path(id): Path<String>,
    Json(req): Json<UpdateOrgRequest>,
) -> AppResult<Json<Value>> {
    assert_is_org_admin(&pool, &id, &user.0.sub).await?;

    if let Some(name) = &req.name {
        sqlx::query("UPDATE organizations SET name = ?, updated_at = datetime('now') WHERE id = ?")
            .bind(name).bind(&id).execute(&pool).await?;
    }
    if let Some(plan) = &req.plan {
        if !matches!(plan.as_str(), "free" | "pro" | "enterprise") {
            return Err(AppError::BadRequest("plan must be free, pro, or enterprise".into()));
        }
        sqlx::query("UPDATE organizations SET plan = ?, updated_at = datetime('now') WHERE id = ?")
            .bind(plan).bind(&id).execute(&pool).await?;
    }
    Ok(Json(json!({ "success": true })))
}

/// Delete an org (owner only).
pub async fn delete_org(
    State(pool): State<SqlitePool>,
    user: DashboardUser,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    let row: Option<(String,)> = sqlx::query_as("SELECT owner_user_id FROM organizations WHERE id = ?")
        .bind(&id)
        .fetch_optional(&pool)
        .await?;
    let (owner_id,) = row.ok_or_else(|| AppError::NotFound("org not found".into()))?;
    if owner_id != user.0.sub {
        return Err(AppError::Forbidden("only the owner can delete an org".into()));
    }
    sqlx::query("DELETE FROM organizations WHERE id = ?")
        .bind(&id).execute(&pool).await?;
    Ok(Json(json!({ "success": true })))
}

/// Add a member to an org.
pub async fn add_member(
    State(pool): State<SqlitePool>,
    user: DashboardUser,
    Path(id): Path<String>,
    Json(req): Json<AddMemberRequest>,
) -> AppResult<Json<Value>> {
    assert_is_org_admin(&pool, &id, &user.0.sub).await?;
    let role = req.role.unwrap_or_else(|| "member".into());
    if !matches!(role.as_str(), "admin" | "member") {
        return Err(AppError::BadRequest("role must be admin or member".into()));
    }
    sqlx::query("INSERT OR IGNORE INTO organization_members (org_id, user_id, role) VALUES (?, ?, ?)")
        .bind(&id).bind(&req.user_id).bind(&role).execute(&pool).await?;
    Ok(Json(json!({ "success": true })))
}

/// Remove a member from an org.
pub async fn remove_member(
    State(pool): State<SqlitePool>,
    user: DashboardUser,
    Path((id, user_id)): Path<(String, String)>,
) -> AppResult<Json<Value>> {
    assert_is_org_admin(&pool, &id, &user.0.sub).await?;
    if user_id == user.0.sub {
        return Err(AppError::BadRequest("cannot remove yourself; transfer ownership first".into()));
    }
    sqlx::query("DELETE FROM organization_members WHERE org_id = ? AND user_id = ?")
        .bind(&id).bind(&user_id).execute(&pool).await?;
    Ok(Json(json!({ "success": true })))
}

/// Get org usage stats.
pub async fn org_usage(
    State(pool): State<SqlitePool>,
    user: DashboardUser,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    assert_is_org_member(&pool, &id, &user.0.sub).await?;
    let total_requests: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM usage_logs WHERE org_id = ?")
        .bind(&id).fetch_one(&pool).await.unwrap_or(0);
    let total_tokens: i64 = sqlx::query_scalar("SELECT COALESCE(SUM(total_tokens), 0) FROM usage_logs WHERE org_id = ?")
        .bind(&id).fetch_one(&pool).await.unwrap_or(0);
    let total_cost: f64 = sqlx::query_scalar("SELECT COALESCE(SUM(cost_usd), 0) FROM usage_logs WHERE org_id = ?")
        .bind(&id).fetch_one(&pool).await.unwrap_or(0.0);
    let error_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM usage_logs WHERE org_id = ? AND status_code >= 400")
        .bind(&id).fetch_one(&pool).await.unwrap_or(0);
    Ok(Json(json!({
        "total_requests": total_requests,
        "total_tokens": total_tokens,
        "total_cost_usd": total_cost,
        "error_count": error_count,
    })))
}

async fn assert_is_org_member(pool: &SqlitePool, org_id: &str, user_id: &str) -> AppResult<()> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT user_id FROM organization_members WHERE org_id = ? AND user_id = ?"
    )
    .bind(org_id).bind(user_id).fetch_optional(pool).await?;
    if row.is_none() {
        return Err(AppError::Forbidden("not a member of this org".into()));
    }
    Ok(())
}

async fn assert_is_org_admin(pool: &SqlitePool, org_id: &str, user_id: &str) -> AppResult<()> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT role FROM organization_members WHERE org_id = ? AND user_id = ?",
    )
    .bind(org_id).bind(user_id).fetch_optional(pool).await?;
    match row {
        Some((role,)) if role == "owner" || role == "admin" => Ok(()),
        _ => Err(AppError::Forbidden("admin access required".into())),
    }
}
