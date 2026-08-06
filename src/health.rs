//! Per-provider health monitoring — background task that periodically pings
//! each configured provider with a minimal request and tracks success/failure.
//!
//! Providers that fail N consecutive pings are marked "degraded" — the
//! failover layer skips them until they recover.
//!
//! Health is exposed via:
//!   GET /api/dashboard/health        — per-provider status
//!   GET /api/dashboard/health/:id    — single provider status
//!
//! And broadcast over the live WS dashboard as ProviderStatus events.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use once_cell::sync::Lazy;
use serde::Serialize;
use tokio::sync::RwLock;

use crate::providers::{ProviderId, Registry};

#[derive(Debug, Clone, Serialize)]
pub struct ProviderHealth {
    pub provider_id: String,
    pub status: HealthStatus,
    pub last_check: Option<String>,    // ISO 8601
    pub last_success: Option<String>,
    pub last_error: Option<String>,
    pub consecutive_failures: u32,
    pub latency_ms: Option<u64>,
    pub total_checks: u64,
    pub total_failures: u64,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub enum HealthStatus {
    Healthy,
    Degraded,    // some failures
    Down,        // N consecutive failures
    Unknown,     // not checked yet
}

pub static HEALTH: Lazy<RwLock<HashMap<ProviderId, ProviderHealth>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

const DEGRADED_THRESHOLD: u32 = 2;
const DOWN_THRESHOLD: u32 = 5;
const CHECK_INTERVAL_SECS: u64 = 300; // 5 minutes

/// Start the background health monitor. Spawns a tokio task that pings
/// every configured provider every 5 minutes.
pub fn start_monitor(registry: Arc<RwLock<Registry>>) {
    tokio::spawn(async move {
        // Initial check after 30s (let the server warm up)
        tokio::time::sleep(Duration::from_secs(30)).await;

        loop {
            let reg = registry.read().await;
            let providers = reg.all();
            drop(reg);

            for provider in providers {
                let pid = provider.id();
                // Spawn each check in parallel
                let provider_clone = provider.clone();
                tokio::spawn(async move {
                    check_provider(provider_clone).await;
                });
                let _ = pid;
            }

            tokio::time::sleep(Duration::from_secs(CHECK_INTERVAL_SECS)).await;
        }
    });
}

async fn check_provider(provider: Arc<dyn crate::providers::Provider>) {
    let pid = provider.id();
    let start = Instant::now();

    // Send a minimal chat request — 1 token, cheapest model
    let req = crate::models::chat::ChatCompletionRequest {
        model: format!("{}:test", pid),
        messages: vec![crate::models::chat::Message {
            role: "user".into(),
            content: Some(crate::models::chat::MessageContent::Text("hi".into())),
            tool_calls: None, tool_call_id: None, name: None,
        }],
        temperature: 0.0,
        top_p: 1.0,
        max_tokens: Some(1),
        stream: false,
        stop: None, seed: None, tools: None, tool_choice: None,
        extra: serde_json::Map::new(),
    };

    let result = provider.chat(&req).await;
    let latency_ms = start.elapsed().as_millis() as u64;
    let now = chrono::Utc::now().to_rfc3339();

    let mut health_map = HEALTH.write().await;
    let entry = health_map.entry(pid).or_insert_with(|| ProviderHealth {
        provider_id: pid.to_string(),
        status: HealthStatus::Unknown,
        last_check: None,
        last_success: None,
        last_error: None,
        consecutive_failures: 0,
        latency_ms: None,
        total_checks: 0,
        total_failures: 0,
    });

    entry.last_check = Some(now.clone());
    entry.total_checks += 1;
    entry.latency_ms = Some(latency_ms);

    match result {
        Ok(_) => {
            entry.last_success = Some(now);
            entry.last_error = None;
            entry.consecutive_failures = 0;
            entry.status = HealthStatus::Healthy;
            crate::live::broadcast_provider_status(pid.as_str(), "ok", None);
        }
        Err(e) => {
            let err_msg = e.to_string();
            entry.last_error = Some(err_msg.clone());
            entry.consecutive_failures += 1;
            entry.total_failures += 1;
            entry.status = if entry.consecutive_failures >= DOWN_THRESHOLD {
                HealthStatus::Down
            } else if entry.consecutive_failures >= DEGRADED_THRESHOLD {
                HealthStatus::Degraded
            } else {
                HealthStatus::Healthy
            };
            crate::live::broadcast_provider_status(
                pid.as_str(),
                if entry.status == HealthStatus::Down { "down" } else { "degraded" },
                Some(err_msg),
            );
        }
    }
}

/// Get the health status of all providers.
pub async fn list_all() -> Vec<ProviderHealth> {
    HEALTH.read().await.values().cloned().collect()
}

/// Get the health status of a single provider.
pub async fn get(id: ProviderId) -> Option<ProviderHealth> {
    HEALTH.read().await.get(&id).cloned()
}

/// Is the provider currently healthy enough to handle requests?
/// (Down providers are skipped by the failover layer.)
pub async fn is_healthy(id: ProviderId) -> bool {
    HEALTH.read().await
        .get(&id)
        .map(|h| h.status != HealthStatus::Down)
        .unwrap_or(true) // unknown = assume healthy
}
