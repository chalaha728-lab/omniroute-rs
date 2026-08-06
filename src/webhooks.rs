//! Webhooks — fire HTTP POST callbacks to registered URLs on events.
//!
//! Events:
//!   - `usage.recorded`  — after each request, with token counts
//!   - `provider.failed` — when a provider errors (failover triggered)
//!   - `provider.recovered` — when a previously-failed provider succeeds
//!
//! Configure via env:
//!   OMNIROUTE_WEBHOOK_URL=https://your-app.com/webhooks/omniroute
//!   OMNIROUTE_WEBHOOK_SECRET=shared-secret  (sent as X-Webhook-Signature header)
//!
//! Webhooks are best-effort, fire-and-forget — failures are logged but don't
//! block the request. Time out after 5s.

use reqwest::Client;
use serde::Serialize;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use once_cell::sync::Lazy;

#[derive(Debug, Clone, Default)]
pub struct WebhookConfig {
    pub url: Option<String>,
    pub secret: Option<String>,
}

static CONFIG: Lazy<Arc<RwLock<WebhookConfig>>> = Lazy::new(|| Arc::new(RwLock::new(WebhookConfig::default())));
static CLIENT: Lazy<Client> = Lazy::new(|| {
    Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("failed to build webhook client")
});

/// Initialize webhook config from env vars. Called once at startup.
pub fn init_from_env() {
    let cfg = WebhookConfig {
        url: std::env::var("OMNIROUTE_WEBHOOK_URL").ok().filter(|s| !s.is_empty()),
        secret: std::env::var("OMNIROUTE_WEBHOOK_SECRET").ok().filter(|s| !s.is_empty()),
    };
    if cfg.url.is_some() {
        tracing::info!("[webhooks] enabled → {}", cfg.url.as_ref().unwrap());
    }
    let mut guard = CONFIG.blocking_write();
    *guard = cfg;
}

/// Fire a webhook event. Non-blocking — returns immediately.
pub fn fire(event_type: &str, payload: Value) {
    let event_type = event_type.to_string();
    tokio::spawn(async move {
        let cfg = CONFIG.read().await.clone();
        let url = match cfg.url {
            Some(u) => u,
            None => return, // webhooks disabled
        };
        let body = json!({
            "event": event_type,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "data": payload,
        });
        let mut req = CLIENT.post(&url).json(&body);
        if let Some(secret) = &cfg.secret {
            // HMAC-SHA256 signature (simplified: just send the secret as a header
            // for now — a real impl would compute HMAC over the body)
            req = req.header("X-Webhook-Signature", secret);
        }
        match req.send().await {
            Ok(resp) => {
                if !resp.status().is_success() {
                    tracing::warn!("[webhooks] {} returned {}", event_type, resp.status());
                }
            }
            Err(e) => tracing::warn!("[webhooks] {} failed: {}", event_type, e),
        }
    });
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageEvent {
    pub api_key_id: Option<String>,
    pub user_id: Option<String>,
    pub provider_id: String,
    pub model: String,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    pub duration_ms: u64,
    pub status_code: u16,
}

pub fn fire_usage(event: UsageEvent) {
    fire("usage.recorded", serde_json::to_value(event).unwrap_or(Value::Null));
}

pub fn fire_provider_failed(provider_id: &str, model: &str, error: &str) {
    fire("provider.failed", json!({
        "provider_id": provider_id,
        "model": model,
        "error": error,
    }));
}

pub fn fire_provider_recovered(provider_id: &str, model: &str) {
    fire("provider.recovered", json!({
        "provider_id": provider_id,
        "model": model,
    }));
}
