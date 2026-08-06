//! Rate limiting — per-API-key AND per-IP token bucket.
//!
//! Two limits, both configurable via env vars:
//!   - OMNIROUTE_RATE_LIMIT_PER_KEY  (default: 100 req/min)  — per API key
//!   - OMNIROUTE_RATE_LIMIT_PER_IP   (default: 60 req/min)   — per client IP
//!
//! Set either to 0 to disable that dimension.
//!
//! Uses an in-memory token bucket (no Redis dependency). Resets on restart.
//! For multi-instance deployments, swap this for a Redis-backed implementation.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use once_cell::sync::Lazy;

use crate::error::AppError;

#[derive(Debug, Clone)]
struct Bucket {
    tokens: f64,
    last_refill: Instant,
    capacity: f64,
    refill_per_sec: f64,
}

impl Bucket {
    fn new(capacity: f64, refill_per_sec: f64) -> Self {
        Self {
            tokens: capacity,
            last_refill: Instant::now(),
            capacity,
            refill_per_sec,
        }
    }

    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.refill_per_sec).min(self.capacity);
        self.last_refill = now;
    }

    fn try_consume(&mut self) -> bool {
        self.refill();
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

struct RateLimiter {
    per_key: HashMap<String, Bucket>,
    per_ip: HashMap<String, Bucket>,
    per_key_capacity: f64,
    per_key_refill: f64,
    per_ip_capacity: f64,
    per_ip_refill: f64,
}

impl RateLimiter {
    fn from_env() -> Self {
        let per_key_rpm = std::env::var("OMNIROUTE_RATE_LIMIT_PER_KEY")
            .ok().and_then(|v| v.parse().ok()).unwrap_or(100u32);
        let per_ip_rpm = std::env::var("OMNIROUTE_RATE_LIMIT_PER_IP")
            .ok().and_then(|v| v.parse().ok()).unwrap_or(60u32);

        Self {
            per_key: HashMap::new(),
            per_ip: HashMap::new(),
            per_key_capacity: per_key_rpm as f64,
            per_key_refill: per_key_rpm as f64 / 60.0,
            per_ip_capacity: per_ip_rpm as f64,
            per_ip_refill: per_ip_rpm as f64 / 60.0,
        }
    }

    fn check_key(&mut self, key: &str) -> Result<(), AppError> {
        if self.per_key_capacity == 0.0 {
            return Ok(());
        }
        let bucket = self.per_key.entry(key.to_string())
            .or_insert_with(|| Bucket::new(self.per_key_capacity, self.per_key_refill));
        if bucket.try_consume() {
            Ok(())
        } else {
            Err(AppError::RateLimited(format!(
                "per-key rate limit exceeded ({} req/min)", self.per_key_capacity as u32
            )))
        }
    }

    fn check_ip(&mut self, ip: &str) -> Result<(), AppError> {
        if self.per_ip_capacity == 0.0 {
            return Ok(());
        }
        let bucket = self.per_ip.entry(ip.to_string())
            .or_insert_with(|| Bucket::new(self.per_ip_capacity, self.per_ip_refill));
        if bucket.try_consume() {
            Ok(())
        } else {
            Err(AppError::RateLimited(format!(
                "per-IP rate limit exceeded ({} req/min)", self.per_ip_capacity as u32
            )))
        }
    }

    /// Garbage-collect buckets that haven't been touched in 10 minutes.
    /// Called periodically to prevent unbounded memory growth.
    fn gc(&mut self) {
        let cutoff = Instant::now() - Duration::from_secs(600);
        self.per_key.retain(|_, b| b.last_refill > cutoff);
        self.per_ip.retain(|_, b| b.last_refill > cutoff);
    }
}

static LIMITER: Lazy<Mutex<RateLimiter>> = Lazy::new(|| Mutex::new(RateLimiter::from_env()));

/// Check both per-key and per-IP rate limits. Returns Err(429) if exceeded.
pub fn check(api_key_id: Option<&str>, client_ip: Option<&str>) -> Result<(), AppError> {
    let mut limiter = LIMITER.lock().unwrap();

    if let Some(key) = api_key_id {
        limiter.check_key(key)?;
    }
    if let Some(ip) = client_ip {
        limiter.check_ip(ip)?;
    }

    // Opportunistic GC — every ~1000 requests, clean up stale buckets.
    // (Cheap heuristic: count total buckets; if >10k, run GC.)
    if limiter.per_key.len() + limiter.per_ip.len() > 10_000 {
        limiter.gc();
    }

    Ok(())
}

/// Parse the client IP from a request's headers (X-Forwarded-For, X-Real-IP, or socket addr).
pub fn extract_client_ip(headers: &axum::http::HeaderMap, connect_info: Option<std::net::SocketAddr>) -> Option<String> {
    // X-Forwarded-For: client, proxy1, proxy2 — take the first
    if let Some(xff) = headers.get("x-forwarded-for") {
        if let Ok(s) = xff.to_str() {
            if let Some(first) = s.split(',').next() {
                let ip = first.trim();
                if !ip.is_empty() {
                    return Some(ip.to_string());
                }
            }
        }
    }
    // X-Real-IP
    if let Some(xri) = headers.get("x-real-ip") {
        if let Ok(s) = xri.to_str() {
            return Some(s.trim().to_string());
        }
    }
    // Fall back to socket addr
    if let Some(addr) = connect_info {
        return Some(addr.ip().to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bucket_allows_burst_then_throttles() {
        let mut b = Bucket::new(5.0, 5.0 / 60.0); // 5 req/min
        for _ in 0..5 {
            assert!(b.try_consume());
        }
        // 6th should fail (bucket empty)
        assert!(!b.try_consume());
    }

    #[test]
    fn bucket_refills_over_time() {
        let mut b = Bucket::new(1.0, 60.0); // 1 capacity, 60/sec refill (fast for test)
        assert!(b.try_consume());
        assert!(!b.try_consume());
        std::thread::sleep(Duration::from_millis(50));
        assert!(b.try_consume()); // refilled
    }
}
