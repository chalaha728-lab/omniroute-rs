//! SQLite connection pool + migrations.

use std::path::Path;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

use crate::config::Config;
use crate::error::AppResult;

/// Create a SQLite connection pool and run migrations.
pub async fn init(config: &Config) -> AppResult<SqlitePool> {
    tracing::info!("[db] initializing at {}", config.data_dir.join("omniroute.db").display());

    let opts = SqliteConnectOptions::new()
        .filename(&config.data_dir.join("omniroute.db"))
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
        .foreign_keys(true)
        .busy_timeout(std::time::Duration::from_secs(5));

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .acquire_timeout(std::time::Duration::from_secs(10))
        .connect_with(opts)
        .await?;

    // Run embedded migrations. If a previous run left a partially-applied
    // migration, delete the DB and retry.
    if let Err(e) = sqlx::migrate!("./migrations").run(&pool).await {
        tracing::warn!("[db] migration failed ({}), deleting DB and retrying...", e);
        drop(pool);
        let db_path = config.data_dir.join("omniroute.db");
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(format!("{}-wal", db_path.display()));
        let _ = std::fs::remove_file(format!("{}-shm", db_path.display()));
        let opts2 = SqliteConnectOptions::new()
            .filename(&db_path)
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
            .foreign_keys(true)
            .busy_timeout(std::time::Duration::from_secs(5));
        let pool2 = SqlitePoolOptions::new()
            .max_connections(5)
            .acquire_timeout(std::time::Duration::from_secs(10))
            .connect_with(opts2)
            .await?;
        sqlx::migrate!("./migrations").run(&pool2).await.map_err(|e| {
            crate::error::AppError::Internal(format!("migration failed after DB reset: {}", e))
        })?;
        tracing::info!("[db] recovered after DB reset");
        seed_defaults(&pool2, config).await?;
        tracing::info!("[db] ready");
        return Ok(pool2);
    }

    // Seed the admin user + default providers on first run
    seed_defaults(&pool, config).await?;

    tracing::info!("[db] ready");
    Ok(pool)
}

/// Seed the admin user (if not exists) and default provider rows.
async fn seed_defaults(pool: &SqlitePool, config: &Config) -> AppResult<()> {
    // Admin user
    let existing: Option<(String,)> = sqlx::query_as("SELECT id FROM users WHERE username = ?")
        .bind("admin")
        .fetch_optional(pool)
        .await?;

    if existing.is_none() {
        let id = uuid::Uuid::new_v4().to_string();
        let password_hash = crate::auth::hash_password(&config.initial_password)
            .map_err(|e| crate::error::AppError::Internal(format!("password hash failed: {}", e)))?;
        sqlx::query("INSERT INTO users (id, username, password_hash, role) VALUES (?, ?, ?, 'admin')")
            .bind(&id)
            .bind("admin")
            .bind(&password_hash)
            .execute(pool)
            .await?;
        tracing::info!("[db] seeded admin user (username=admin, password from INITIAL_PASSWORD)");
    }

    // Default provider rows (priority 100 = first)
    let providers = [
        ("openai", "OpenAI", 100),
        ("anthropic", "Anthropic (Claude)", 110),
        ("gemini", "Google Gemini", 120),
        ("deepseek", "DeepSeek", 130),
        ("openrouter", "OpenRouter", 140),
    ];
    for (id, name, priority) in providers {
        sqlx::query(
            "INSERT OR IGNORE INTO providers (id, display_name, priority) VALUES (?, ?, ?)",
        )
        .bind(id)
        .bind(name)
        .bind(priority)
        .execute(pool)
        .await?;
    }

    Ok(())
}

/// Quick health-check: can we ping the DB?
pub async fn ping(pool: &SqlitePool) -> bool {
    sqlx::query_scalar::<_, i64>("SELECT 1")
        .fetch_one(pool)
        .await
        .is_ok()
}

#[allow(dead_code)]
pub fn db_path(config: &Config) -> std::path::PathBuf {
    Path::new(&config.data_dir).join("omniroute.db")
}
