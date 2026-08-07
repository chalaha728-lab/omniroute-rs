//! OmniRoute-Rust — fast, light, OpenAI-compatible AI gateway.
//!
//! Single binary, no runtime deps. ~15-20 MB stripped.

// Bump the macro recursion limit — the OpenAPI spec in src/openapi.rs is a
// 200+ line nested json!({...}) literal that exceeds the default limit of 128.
#![recursion_limit = "1024"]

mod a2a;
mod auth;
mod aws;
mod cache;
mod cli;
mod compression;
mod config;
mod db;
mod error;
mod guardrails;
mod health;
mod i18n;
mod live;
mod mcp;
mod metrics;
mod middleware;
mod models;
mod openapi;
mod plugins;
mod pricing;
mod providers;
mod rate_limit;
mod routes;
mod systemd;
mod tenant;
mod tenant_quota;
mod tunnel;
mod webhooks;

/// Globally-shared server port — used by tunnel module.
use once_cell::sync::Lazy;
use std::sync::Mutex;
pub static SERVER_PORT: Lazy<Mutex<u16>> = Lazy::new(|| Mutex::new(20128));

use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::State;
use axum::routing::{delete, get, post, put};
use axum::Json;
use axum::Router;
use clap::{Parser, Subcommand};
use middleware::auth::{ApiKeyAuth, DashboardUser};
use sqlx::SqlitePool;
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

use crate::config::Config;
use crate::providers::{Registry, SharedRegistry};

/// OmniRoute — fast, light, OpenAI-compatible AI gateway.
#[derive(Parser, Debug)]
#[command(name = "omniroute", version, about, long_about = None)]
struct Cli {
    /// Subcommand. If omitted, runs the HTTP server.
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run the MCP server over stdio (for Claude Desktop / Cursor integration).
    Mcp,

    /// Run pending DB migrations and exit.
    Migrate,

    /// Reset a user's password.
    ResetPassword {
        /// Username to reset.
        #[arg(long)]
        username: String,
        /// New password (min 8 chars). If omitted, you'll be prompted interactively.
        #[arg(long)]
        password: Option<String>,
    },

    /// Create a new user.
    CreateUser {
        #[arg(long)]
        username: String,
        #[arg(long)]
        password: String,
        #[arg(long, default_value = "member")]
        role: String,
    },

    /// List all users.
    ListUsers,

    /// List all API keys.
    ListKeys,

    /// Generate a random secret (for JWT_SECRET or API_KEY_SECRET).
    GenSecret {
        /// Number of random bytes (default 32).
        #[arg(long, default_value = "32")]
        bytes: usize,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let config = Config::from_env()?;

    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&config.log_level)),
        )
        .with_target(false)
        .compact()
        .init();

    // Init DB + run migrations
    let pool = db::init(&config).await?;
    let registry: SharedRegistry = Arc::new(RwLock::new(Registry::build(&config)));

    // Register built-in plugins (quota plugin is opt-in via OMNIROUTE_QUOTA_ENABLED)
    {
        let mut plugins = plugins::PLUGINS.write().await;
        plugins.register(Arc::new(plugins::quota::QuotaPlugin::new(Arc::new(pool.clone()))));
        tracing::info!("[plugins] registered {} plugins", plugins.list().len());
    }

    // Start the background health monitor (pings every 5 min, broadcasts on WS)
    health::start_monitor(registry.clone());

    // Initialize webhooks (if configured)
    webhooks::init_from_env();

    // Sync global SERVER_PORT for use by other modules (tunnel, etc.)
    *SERVER_PORT.lock().unwrap() = config.port;

    match cli.command {
        Some(Commands::Mcp) => {
            tracing::info!("[mcp] starting stdio transport");
            mcp::transport::run_stdio(registry.clone(), pool).await?;
        }
        Some(Commands::Migrate) => {
            cli::migrate(&pool).await?;
        }
        Some(Commands::ResetPassword { username, password }) => {
            let pwd = password.unwrap_or_else(|| {
                use std::io::{self, Write};
                eprint!("Enter new password for {}: ", username);
                io::stderr().flush().ok();
                let mut input = String::new();
                io::stdin().read_line(&mut input).ok();
                input.trim().to_string()
            });
            cli::reset_password(&pool, &username, &pwd).await?;
        }
        Some(Commands::CreateUser { username, password, role }) => {
            cli::create_user(&pool, &username, &password, &role).await?;
        }
        Some(Commands::ListUsers) => {
            cli::list_users(&pool).await?;
        }
        Some(Commands::ListKeys) => {
            cli::list_keys(&pool).await?;
        }
        Some(Commands::GenSecret { bytes }) => {
            cli::gen_secret(bytes);
        }
        None => {
            tracing::info!("OmniRoute-Rust v{} starting", env!("CARGO_PKG_VERSION"));
            tracing::info!("data_dir: {}", config.data_dir.display());
            tracing::info!("listen: {}:{}", config.host, config.port);

            let app = build_router(config.clone(), pool.clone(), registry.clone());
            let addr: SocketAddr = config.listen_addr().parse()?;
            tracing::info!("✓ ready — http://{}", addr);
            let listener = tokio::net::TcpListener::bind(addr).await?;
            axum::serve(listener, app).await?;
        }
    }

    Ok(())
}

fn build_router(config: Config, pool: SqlitePool, registry: SharedRegistry) -> Router {
    // ─── /v1/* — OpenAI-compatible API (api key OR jwt) ─────────────────────
    let v1_api = Router::new()
        .route("/chat/completions", post(routes::chat::chat_completions))
        .route("/models", get(routes::models::list_models))
        .route("/embeddings", post(routes::embeddings::create_embedding))
        .route("/audio/speech", post(routes::audio::create_speech))
        .route("/images/generations", post(routes::images::create_image))
        .route("/mcp/sse", get(mcp::transport::sse_endpoint))
        .route("/mcp/messages", post(mcp::transport::messages_endpoint))
        .layer(TraceLayer::new_for_http());

    // ─── /api/auth/* — dashboard auth ──────────────────────────────────────
    let auth_routes = Router::new()
        .route("/login", post(routes::auth::login))
        .route("/verify", get(routes::auth::verify))
        .route("/password", post(routes::auth::change_password));

    // ─── /api/dashboard/* — dashboard API (jwt required) ───────────────────
    let dashboard = Router::new()
        .route("/usage", get(routes::dashboard::usage_summary))
        .route("/providers", get(routes::dashboard::list_providers))
        .route("/providers/:id", put(routes::dashboard::update_provider))
        .route("/api-keys", get(routes::dashboard::list_api_keys).post(routes::dashboard::create_api_key))
        .route("/api-keys/:id", delete(routes::dashboard::delete_api_key))
        .route("/api-keys/:id/quota", get(get_api_key_quota).put(set_api_key_quota))
        .route("/whoami", get(routes::dashboard::whoami))
        .route("/systemd/install", post(systemd::install))
        .route("/systemd/uninstall", post(systemd::uninstall))
        .route("/systemd/status", get(systemd::status))
        .route("/tunnel/start", post(tunnel::start))
        .route("/tunnel/stop", post(tunnel::stop))
        .route("/tunnel/status", get(tunnel::status))
        .route("/plugins", get(list_plugins))
        .route("/health", get(list_provider_health))
        .route("/health/:id", get(get_provider_health))
        .route("/pricing", get(list_pricing))
        .route("/pricing/:provider/:model", put(set_pricing_override))
        // Multi-tenant: organizations
        .route("/orgs", get(tenant::list_orgs).post(tenant::create_org))
        .route("/orgs/:id", get(tenant::get_org).put(tenant::update_org).delete(tenant::delete_org))
        .route("/orgs/:id/members", post(tenant::add_member))
        .route("/orgs/:id/members/:user_id", delete(tenant::remove_member))
        .route("/orgs/:id/usage", get(tenant::org_usage));

    // ─── /v1/a2a/* — Agent-to-Agent protocol ───────────────────────────────
    let a2a_routes = Router::new()
        .route("/agents", get(a2a::routes::list_agents).post(a2a::routes::register_agent))
        .route("/agents/:id", get(a2a::routes::get_agent).delete(a2a::routes::delete_agent))
        .route("/agents/:id/invoke", post(a2a::routes::invoke_agent));

    // ─── /ws/* — WebSocket endpoints ───────────────────────────────────────
    let ws_routes = Router::new()
        .route("/dashboard", get(live::ws_dashboard));

    // ─── /api/monitoring/* — health checks (no auth) ───────────────────────
    let monitoring = Router::new()
        .route("/health", get(routes::health::health));

    // ─── /metrics — Prometheus metrics (no auth) ───────────────────────────
    // ─── /api/openapi.json — OpenAPI 3.0 spec (no auth) ────────────────────
    // ─── /dashboard-demo.html — Live WS dashboard demo (no auth) ───────────
    let misc = Router::new()
        .route("/metrics", get(metrics::metrics_handler))
        .route("/api/openapi.json", get(openapi::openapi_spec))
        .route("/dashboard-demo.html", get(dashboard_demo_html));

    // ─── Static dashboard (optional) ───────────────────────────────────────
    let static_layer = config.dashboard_dist.clone().map(|dist| {
        Router::new()
            .nest_service("/", ServeDir::new(dist).append_index_html_on_directories(true))
            .fallback(serve_dashboard_index)
    });

    // ─── Compose ────────────────────────────────────────────────────────────
    let app_state = AppState {
        pool: pool.clone(),
        config: config.clone(),
        registry: registry.clone(),
    };

    let mut app = Router::new()
        .nest("/v1", v1_api)
        .nest("/v1/a2a", a2a_routes)
        .nest("/ws", ws_routes)
        .nest("/api/auth", auth_routes)
        .nest("/api/dashboard", dashboard)
        .nest("/api/monitoring", monitoring)
        .merge(misc)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(app_state);

    if let Some(static_router) = static_layer {
        app = app.merge(static_router.with_state(()));
    }

    app
}

/// Unified app state — handlers extract State<SqlitePool>, State<Config>,
/// State<SharedRegistry> via the FromRef impls below.
#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub config: Config,
    pub registry: SharedRegistry,
}

impl axum::extract::FromRef<AppState> for SqlitePool {
    fn from_ref(state: &AppState) -> Self {
        state.pool.clone()
    }
}

impl axum::extract::FromRef<AppState> for Config {
    fn from_ref(state: &AppState) -> Self {
        state.config.clone()
    }
}

impl axum::extract::FromRef<AppState> for SharedRegistry {
    fn from_ref(state: &AppState) -> Self {
        state.registry.clone()
    }
}

/// Fallback handler — serves index.html for client-side routing (e.g. /dashboard/*).
async fn serve_dashboard_index() -> impl axum::response::IntoResponse {
    // For the static export, we'd read from DASHBOARD_DIST/index.html.
    // For now, return a minimal placeholder so the API still works in API-only mode.
    axum::response::Html(
        r#"<!doctype html>
<html><head><title>OmniRoute</title></head>
<body style="font-family:system-ui;padding:2rem;max-width:60ch">
  <h1>OmniRoute-Rust API</h1>
  <p>The API is running. The dashboard is not bundled in this build.</p>
  <h2>Quick test</h2>
  <pre>curl http://localhost:20128/api/monitoring/health</pre>
  <h2>Login</h2>
  <pre>curl -X POST http://localhost:20128/api/auth/login \\
  -H 'Content-Type: application/json' \\
  -d '{"username":"admin","password":"CHANGEME"}'</pre>
  <h2>List models</h2>
  <pre>curl http://localhost:20128/v1/models \\
  -H 'Authorization: Bearer &lt;jwt-or-api-key&gt;'</pre>
  <h2>Chat completion</h2>
  <pre>curl -X POST http://localhost:20128/v1/chat/completions \\
  -H 'Authorization: Bearer &lt;jwt-or-api-key&gt;' \\
  -H 'Content-Type: application/json' \\
  -d '{"model":"openai:gpt-4o-mini","messages":[{"role":"user","content":"Hello"}]}'</pre>
</body></html>"#,
    )
}

// Suppress unused warnings for extractors that are wired up via axum's extractor
// resolution but not directly called from this file.
#[allow(dead_code)]
fn _extractor_types() -> (DashboardUser, ApiKeyAuth) {
    unreachable!()
}

/// List installed plugins. GET /api/dashboard/plugins
async fn list_plugins(_user: DashboardUser) -> Result<Json<serde_json::Value>, crate::error::AppError> {
    let reg = crate::plugins::PLUGINS.read().await;
    Ok(Json(serde_json::json!({ "plugins": reg.list() })))
}

/// GET /api/dashboard/health — list health status of all providers
async fn list_provider_health(_user: DashboardUser) -> Result<Json<serde_json::Value>, crate::error::AppError> {
    let health = crate::health::list_all().await;
    Ok(Json(serde_json::json!({ "providers": health })))
}

/// GET /api/dashboard/health/:id — single provider health
async fn get_provider_health(
    _user: DashboardUser,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Json<serde_json::Value>, crate::error::AppError> {
    let pid = crate::models::provider::ProviderId::from_str(&id)
        .ok_or_else(|| crate::error::AppError::BadRequest(format!("unknown provider: {}", id)))?;
    let health = crate::health::get(pid).await
        .ok_or_else(|| crate::error::AppError::NotFound(format!("no health data for: {}", id)))?;
    Ok(Json(serde_json::json!(health)))
}

/// GET /api/dashboard/api-keys/:id/quota — get quota for an API key
async fn get_api_key_quota(
    State(pool): State<SqlitePool>,
    _user: DashboardUser,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Json<serde_json::Value>, crate::error::AppError> {
    let quota = crate::plugins::quota::get_quota_for_api_key(&pool, &id).await;
    Ok(Json(serde_json::json!({ "quota": quota })))
}

/// PUT /api/dashboard/api-keys/:id/quota — set quota for an API key
async fn set_api_key_quota(
    State(pool): State<SqlitePool>,
    _user: DashboardUser,
    axum::extract::Path(id): axum::extract::Path<String>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, crate::error::AppError> {
    let limit = body.get("limit").and_then(|v| v.as_u64())
        .ok_or_else(|| crate::error::AppError::BadRequest("missing 'limit' field".into()))?;
    let window_days = body.get("window_days").and_then(|v| v.as_u64()).unwrap_or(30) as u32;
    let quota = crate::plugins::quota::set_quota_for_api_key(&pool, &id, limit, window_days).await?;
    Ok(Json(serde_json::json!({ "success": true, "quota": quota })))
}

/// GET /api/dashboard/pricing — list all known prices
async fn list_pricing(_user: DashboardUser) -> Result<Json<serde_json::Value>, crate::error::AppError> {
    let prices = crate::pricing::list_all();
    Ok(Json(serde_json::json!({ "prices": prices })))
}

/// PUT /api/dashboard/pricing/:provider/:model — override a price
async fn set_pricing_override(
    _user: DashboardUser,
    axum::extract::Path((provider, model)): axum::extract::Path<(String, String)>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, crate::error::AppError> {
    let pid = crate::models::provider::ProviderId::from_str(&provider)
        .ok_or_else(|| crate::error::AppError::BadRequest(format!("unknown provider: {}", provider)))?;
    let prompt_per_mtok = body.get("prompt_per_mtok").and_then(|v| v.as_f64())
        .ok_or_else(|| crate::error::AppError::BadRequest("missing 'prompt_per_mtok' field".into()))?;
    let completion_per_mtok = body.get("completion_per_mtok").and_then(|v| v.as_f64())
        .ok_or_else(|| crate::error::AppError::BadRequest("missing 'completion_per_mtok' field".into()))?;
    let price = crate::pricing::Price { prompt_per_mtok, completion_per_mtok };
    crate::pricing::set_override(pid, &model, price);
    Ok(Json(serde_json::json!({ "success": true, "price": price })))
}

/// GET /dashboard-demo.html — Live WebSocket dashboard demo
async fn dashboard_demo_html() -> impl axum::response::IntoResponse {
    use axum::http::header::CONTENT_TYPE;
    let html = include_str!("../static/dashboard-demo.html");
    ([(CONTENT_TYPE, "text/html; charset=utf-8")], html)
}
