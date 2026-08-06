//! Cloudflare Tunnel integration — wraps the `cloudflared` CLI to expose the
//! local OmniRoute server publicly via Cloudflare's edge network.
//!
//! Requires `cloudflared` installed on the host:
//!   - macOS: brew install cloudflared
//!   - Linux: apt install cloudflared (or download from cloudflare)
//!   - Windows: winget install Cloudflare.cloudflared
//!
//! Routes:
//!   POST /api/dashboard/tunnel/start   — start a quick tunnel (no account needed)
//!   POST /api/dashboard/tunnel/stop    — stop the running tunnel
//!   GET  /api/dashboard/tunnel/status  — is a tunnel running? what URL?

use std::sync::Mutex;
use once_cell::sync::Lazy;
use axum::Json;
use serde_json::{json, Value};

use crate::error::{AppError, AppResult};
use crate::middleware::auth::DashboardUser;

/// Active tunnel subprocess handle.
struct TunnelState {
    child: Option<std::process::Child>,
    url: Option<String>,
}

static STATE: Lazy<Mutex<TunnelState>> = Lazy::new(|| Mutex::new(TunnelState {
    child: None,
    url: None,
}));

pub async fn start(_user: DashboardUser) -> AppResult<Json<Value>> {
    let port = *crate::SERVER_PORT.lock().unwrap();

    // Check if cloudflared is installed
    let which = std::process::Command::new("which")
        .arg("cloudflared")
        .output()
        .map_err(|e| AppError::Internal(format!("failed to find cloudflared: {}", e)))?;
    if !which.status.success() || which.stdout.is_empty() {
        return Err(AppError::BadRequest(
            "cloudflared is not installed. Install: brew install cloudflared / apt install cloudflared".into()
        ));
    }

    {
        let mut state = STATE.lock().unwrap();
        if state.child.is_some() {
            return Err(AppError::BadRequest("tunnel already running".into()));
        }
    }

    // Start `cloudflared tunnel --url http://localhost:PORT` — quick tunnel, no account needed.
    let child = std::process::Command::new("cloudflared")
        .args(["tunnel", "--url", &format!("http://localhost:{}", port)])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| AppError::Internal(format!("failed to start cloudflared: {}", e)))?;

    let pid = child.id();
    let mut state_guard = STATE.lock().unwrap();
    state_guard.child = Some(child);
    state_guard.url = Some(format!("(starting, pid={})", pid));

    Ok(Json(json!({
        "success": true,
        "pid": pid,
        "note": "Quick tunnel starting. Fetch /api/dashboard/tunnel/status in ~5s to get the public URL.",
    })))
}

pub async fn stop(_user: DashboardUser) -> AppResult<Json<Value>> {
    let mut state = STATE.lock().unwrap();
    let stopped = if let Some(mut child) = state.child.take() {
        let _ = child.kill();
        let _ = child.wait();
        true
    } else {
        false
    };
    state.url = None;
    Ok(Json(json!({ "success": true, "stopped": stopped })))
}

pub async fn status(_user: DashboardUser) -> AppResult<Json<Value>> {
    let state = STATE.lock().unwrap();
    let running = state.child.is_some();
    let url = state.url.clone().unwrap_or_else(|| "none".into());
    Ok(Json(json!({
        "running": running,
        "url": url,
        "note": "If url says '(starting, pid=...)', the tunnel is still initializing. \
                 Cloudflared logs the trycloudflare.com URL on stderr — a full impl would \
                 parse it from the captured log stream."
    })))
}
