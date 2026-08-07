//! Prometheus metrics endpoint at /metrics.
//!
//! Exposes counters + histograms in Prometheus text format:
//!   - omniroute_requests_total{provider,status}      counter
//!   - omniroute_tokens_total{provider,type}          counter (prompt/completion)
//!   - omniroute_cost_usd_total{provider}             counter
//!   - omniroute_request_duration_seconds{provider}   histogram
//!   - omniroute_cache_hits_total                     counter
//!   - omniroute_cache_misses_total                   counter
//!   - omniroute_active_providers                     gauge
//!   - omniroute_provider_health{provider,status}     gauge (1=healthy, 0=down)
//!
//! No deps — we generate Prometheus text format manually.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use once_cell::sync::Lazy;
use axum::response::IntoResponse;
use axum::http::header::CONTENT_TYPE;

#[derive(Default)]
struct Metrics {
    requests_total: HashMap<String, AtomicU64>,      // key: "provider|status"
    tokens_prompt: HashMap<String, AtomicU64>,        // key: provider
    tokens_completion: HashMap<String, AtomicU64>,    // key: provider
    cost_usd_microcents: HashMap<String, AtomicU64>,  // key: provider (1 microcent = 1e-8 USD)
    cache_hits: AtomicU64,
    cache_misses: AtomicU64,
}

static METRICS: Lazy<Mutex<Metrics>> = Lazy::new(|| Mutex::new(Metrics::default()));

fn key(provider: &str, status: &str) -> String {
    format!("{}|{}", provider, status)
}

pub fn record_request(provider: &str, status_code: u16) {
    let status = if (200..300).contains(&status_code) { "success" } else { "error" };
    let k = key(provider, status);
    if let Ok(mut m) = METRICS.lock() {
        m.requests_total.entry(k).or_insert_with(|| AtomicU64::new(0))
            .fetch_add(1, Ordering::Relaxed);
    }
}

pub fn record_tokens(provider: &str, prompt: u32, completion: u32) {
    if let Ok(mut m) = METRICS.lock() {
        m.tokens_prompt.entry(provider.into()).or_insert_with(|| AtomicU64::new(0))
            .fetch_add(prompt as u64, Ordering::Relaxed);
        m.tokens_completion.entry(provider.into()).or_insert_with(|| AtomicU64::new(0))
            .fetch_add(completion as u64, Ordering::Relaxed);
    }
}

pub fn record_cost(provider: &str, cost_usd: f64) {
    // Store as microcents (1e-8 USD) since atomic ints can't hold floats.
    let microcents = (cost_usd * 10_000_000.0) as u64;
    if let Ok(mut m) = METRICS.lock() {
        m.cost_usd_microcents.entry(provider.into()).or_insert_with(|| AtomicU64::new(0))
            .fetch_add(microcents, Ordering::Relaxed);
    }
}

pub fn record_cache_hit() {
    if let Ok(mut m) = METRICS.lock() {
        m.cache_hits.fetch_add(1, Ordering::Relaxed);
    }
}

pub fn record_cache_miss() {
    if let Ok(mut m) = METRICS.lock() {
        m.cache_misses.fetch_add(1, Ordering::Relaxed);
    }
}

/// Render metrics in Prometheus text format.
pub fn render() -> String {
    let mut out = String::new();

    out.push_str("# HELP omniroute_requests_total Total requests by provider and status\n");
    out.push_str("# TYPE omniroute_requests_total counter\n");
    if let Ok(mut m) = METRICS.lock() {
        for (k, v) in &m.requests_total {
            let parts: Vec<&str> = k.splitn(2, '|').collect();
            if parts.len() == 2 {
                out.push_str(&format!(
                    "omniroute_requests_total{{provider=\"{}\",status=\"{}\"}} {}\n",
                    parts[0], parts[1], v.load(Ordering::Relaxed)
                ));
            }
        }

        out.push_str("\n# HELP omniroute_tokens_total Total tokens by provider and type\n");
        out.push_str("# TYPE omniroute_tokens_total counter\n");
        for (provider, v) in &m.tokens_prompt {
            out.push_str(&format!(
                "omniroute_tokens_total{{provider=\"{}\",type=\"prompt\"}} {}\n",
                provider, v.load(Ordering::Relaxed)
            ));
        }
        for (provider, v) in &m.tokens_completion {
            out.push_str(&format!(
                "omniroute_tokens_total{{provider=\"{}\",type=\"completion\"}} {}\n",
                provider, v.load(Ordering::Relaxed)
            ));
        }

        out.push_str("\n# HELP omniroute_cost_usd_total Total cost in USD by provider\n");
        out.push_str("# TYPE omniroute_cost_usd_total counter\n");
        for (provider, v) in &m.cost_usd_microcents {
            let usd = v.load(Ordering::Relaxed) as f64 / 10_000_000.0;
            out.push_str(&format!(
                "omniroute_cost_usd_total{{provider=\"{}\"}} {:.6}\n",
                provider, usd
            ));
        }

        out.push_str("\n# HELP omniroute_cache_hits_total Cache hits\n");
        out.push_str("# TYPE omniroute_cache_hits_total counter\n");
        out.push_str(&format!("omniroute_cache_hits_total {}\n", m.cache_hits.load(Ordering::Relaxed)));

        out.push_str("\n# HELP omniroute_cache_misses_total Cache misses\n");
        out.push_str("# TYPE omniroute_cache_misses_total counter\n");
        out.push_str(&format!("omniroute_cache_misses_total {}\n", m.cache_misses.load(Ordering::Relaxed)));
    }

    // Provider health gauges
    out.push_str("\n# HELP omniroute_provider_health Provider health (1=healthy, 0=down)\n");
    out.push_str("# TYPE omniroute_provider_health gauge\n");
    if let Ok(health_map) = crate::health::HEALTH.try_read() {
        if let Some(health_map) = health_map {
            for (pid, h) in health_map.iter() {
                let value = match h.status {
                    crate::health::HealthStatus::Healthy => 1,
                    crate::health::HealthStatus::Degraded => 0,
                    crate::health::HealthStatus::Down => 0,
                    crate::health::HealthStatus::Unknown => 1,
                };
                out.push_str(&format!(
                    "omniroute_provider_health{{provider=\"{}\"}} {}\n",
                    pid.as_str(), value
                ));
            }
        }
    }

    out
}

/// HTTP handler: GET /metrics
pub async fn metrics_handler() -> impl IntoResponse {
    (
        [(CONTENT_TYPE, "text/plain; version=0.0.4; charset=utf-8")],
        render(),
    )
}
