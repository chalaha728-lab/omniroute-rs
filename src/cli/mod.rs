//! CLI subcommands — admin utilities.
//!
//! Subcommands:
//!   omniroute mcp             — run MCP server over stdio (already implemented)
//!   omniroute migrate         — run DB migrations and exit
//!   omniroute reset-password  — reset the admin password
//!   omniroute create-user     — create a new user
//!   omniroute list-users      — list all users
//!   omniroute list-keys       — list API keys
//!   omniroute gen-secret      — generate a random secret (JWT_SECRET / API_KEY_SECRET)

use sqlx::SqlitePool;
use crate::auth;
use crate::config::Config;
use crate::error::AppError;

/// Run pending DB migrations.
pub async fn migrate(pool: &SqlitePool) -> anyhow::Result<()> {
    println!("[migrate] running migrations...");
    sqlx::migrate!("./migrations").run(pool).await?;
    println!("[migrate] ✓ done");
    Ok(())
}

/// Reset the admin user's password.
pub async fn reset_password(pool: &SqlitePool, username: &str, new_password: &str) -> anyhow::Result<()> {
    if new_password.len() < 8 {
        anyhow::bail!("password must be at least 8 chars");
    }
    let hash = auth::hash_password(new_password)?;
    let result = sqlx::query(
        "UPDATE users SET password_hash = ?, updated_at = datetime('now') WHERE username = ?"
    )
    .bind(&hash)
    .bind(username)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        anyhow::bail!("user not found: {}", username);
    }
    println!("[reset-password] ✓ password updated for user '{}'", username);
    Ok(())
}

/// Create a new user.
pub async fn create_user(
    pool: &SqlitePool,
    username: &str,
    password: &str,
    role: &str,
) -> anyhow::Result<()> {
    if password.len() < 8 {
        anyhow::bail!("password must be at least 8 chars");
    }
    if !matches!(role, "admin" | "member") {
        anyhow::bail!("role must be 'admin' or 'member'");
    }

    // Check if user already exists
    let existing: Option<(String,)> = sqlx::query_as("SELECT id FROM users WHERE username = ?")
        .bind(username)
        .fetch_optional(pool)
        .await?;
    if existing.is_some() {
        anyhow::bail!("user already exists: {}", username);
    }

    let id = uuid::Uuid::new_v4().to_string();
    let hash = auth::hash_password(password)?;
    sqlx::query("INSERT INTO users (id, username, password_hash, role) VALUES (?, ?, ?, ?)")
        .bind(&id)
        .bind(username)
        .bind(&hash)
        .bind(role)
        .execute(pool)
        .await?;
    println!("[create-user] ✓ created user '{}' (role: {}, id: {})", username, role, id);
    Ok(())
}

/// List all users.
pub async fn list_users(pool: &SqlitePool) -> anyhow::Result<()> {
    let rows: Vec<(String, String, String, String)> = sqlx::query_as(
        "SELECT id, username, role, created_at FROM users ORDER BY created_at"
    )
    .fetch_all(pool)
    .await?;

    println!("{:<36}  {:<20}  {:<8}  {}", "ID", "USERNAME", "ROLE", "CREATED");
    println!("{}", "-".repeat(95));
    for (id, username, role, created) in rows {
        println!("{:<36}  {:<20}  {:<8}  {}", id, username, role, created);
    }
    Ok(())
}

/// List all API keys (without revealing the key values).
pub async fn list_keys(pool: &SqlitePool) -> anyhow::Result<()> {
    let rows: Vec<(String, String, String, Option<String>, i64)> = sqlx::query_as(
        r#"SELECT id, name, prefix, last_used_at, enabled FROM api_keys ORDER BY created_at DESC"#
    )
    .fetch_all(pool)
    .await?;

    println!("{:<36}  {:<20}  {:<14}  {:<20}  {}", "ID", "NAME", "PREFIX", "LAST USED", "ENABLED");
    println!("{}", "-".repeat(105));
    for (id, name, prefix, last_used, enabled) in rows {
        println!(
            "{:<36}  {:<20}  {:<14}  {:<20}  {}",
            id, name, prefix, last_used.unwrap_or_else(|| "never".into()), if enabled == 1 { "yes" } else { "no" }
        );
    }
    Ok(())
}

/// Generate a random secret suitable for JWT_SECRET or API_KEY_SECRET.
pub fn gen_secret(bytes: usize) {
    use rand::RngCore;
    let mut buf = vec![0u8; bytes];
    rand::thread_rng().fill_bytes(&mut buf);
    println!("Random {}-byte secret (hex):", bytes);
    println!("{}", hex::encode(&buf));
    println!("\nFor JWT_SECRET (base64):");
    use base64::{engine::general_purpose, Engine};
    println!("{}", general_purpose::STANDARD.encode(&buf));
}

/// Get a SqlitePool from config — convenience for subcommands.
pub async fn pool_from_config(config: &Config) -> Result<SqlitePool, AppError> {
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    let opts = SqliteConnectOptions::new()
        .filename(&config.data_dir.join("omniroute.db"))
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
        .foreign_keys(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(2)
        .connect_with(opts)
        .await?;
    Ok(pool)
}

// base64 is needed for gen-secret. Add to Cargo.toml.
#[allow(unused_imports)]
use base64 as _base64;
