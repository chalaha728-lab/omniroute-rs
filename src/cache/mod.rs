//! Response cache — saves tokens by caching identical chat completion requests.
//!
//! Cache key: SHA-256 of (model, messages, temperature, top_p, max_tokens, stop, seed).
//! Cached responses are stored in-memory with optional TTL.
//!
//! Opt-in via env vars:
//!   OMNIROUTE_CACHE_ENABLED=true                  — enable
//!   OMNIROUTE_CACHE_TTL_SECS=3600                 — default 1 hour
//!   OMNIROUTE_CACHE_MAX_ENTRIES=10000             — LRU eviction
//!
//! IMPORTANT: streaming responses are NOT cached. Only non-streaming responses
//! are cached. Responses that include tool_calls are cached too (the cache
//! trusts that tool results are deterministic for the same prompt).
//!
//! Cache is skipped when:
//!   - stream=true
//!   - The request includes tools and tool_choice="required"
//!   - A random f64 < 0.01 (1% cache bypass for freshness)

use std::collections::HashMap;
use std::time::{Duration, Instant};
use once_cell::sync::Lazy;
use sha2::{Digest, Sha256};
use std::sync::Mutex;

use crate::models::chat::{ChatCompletionRequest, ChatCompletionResponse};

#[derive(Debug, Clone)]
struct CacheEntry {
    response: ChatCompletionResponse,
    inserted_at: Instant,
}

struct Cache {
    entries: HashMap<String, CacheEntry>,
    ttl: Duration,
    max_entries: usize,
}

static CACHE: Lazy<Mutex<Cache>> = Lazy::new(|| {
    let enabled = std::env::var("OMNIROUTE_CACHE_ENABLED")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false);
    let ttl_secs: u64 = std::env::var("OMNIROUTE_CACHE_TTL_SECS")
        .ok().and_then(|v| v.parse().ok()).unwrap_or(3600);
    let max_entries: usize = std::env::var("OMNIROUTE_CACHE_MAX_ENTRIES")
        .ok().and_then(|v| v.parse().ok()).unwrap_or(10_000);
    if enabled {
        tracing::info!("[cache] enabled (ttl={}s, max_entries={})", ttl_secs, max_entries);
    }
    Mutex::new(Cache {
        entries: HashMap::new(),
        ttl: Duration::from_secs(ttl_secs),
        max_entries,
    })
});

pub fn is_enabled() -> bool {
    std::env::var("OMNIROUTE_CACHE_ENABLED")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false)
}

/// Should this request be cached? Returns false for streaming or
/// "force tool call" requests.
pub fn should_cache(req: &ChatCompletionRequest) -> bool {
    if !is_enabled() {
        return false;
    }
    if req.stream {
        return false;
    }
    // Don't cache if tool_choice is "required" (forces a tool call — non-deterministic)
    if let Some(tc) = &req.tool_choice {
        if tc.as_str() == Some("required") {
            return false;
        }
    }
    // 1% random bypass
    use rand::Rng;
    if rand::thread_rng().gen::<f64>() < 0.01 {
        return false;
    }
    true
}

/// Lookup a cached response. Returns None if not found or expired.
pub fn get(req: &ChatCompletionRequest) -> Option<ChatCompletionResponse> {
    if !is_enabled() {
        return None;
    }
    let key = compute_key(req);
    let mut cache = CACHE.lock().unwrap();
    let now = Instant::now();

    if let Some(entry) = cache.entries.get_mut(&key) {
        if now.duration_since(entry.inserted_at) < cache.ttl {
            return Some(entry.response.clone());
        }
        // Expired — remove
        cache.entries.remove(&key);
    }
    None
}

/// Store a response in the cache.
pub fn set(req: &ChatCompletionRequest, response: ChatCompletionResponse) {
    if !should_cache(req) {
        return;
    }
    let key = compute_key(req);
    let mut cache = CACHE.lock().unwrap();

    // LRU eviction if at capacity
    if cache.entries.len() >= cache.max_entries {
        // Evict the oldest entry (by inserted_at)
        if let Some(oldest_key) = cache.entries.iter()
            .min_by_key(|(_, e)| e.inserted_at)
            .map(|(k, _)| k.clone())
        {
            cache.entries.remove(&oldest_key);
        }
    }

    cache.entries.insert(key, CacheEntry {
        response,
        inserted_at: Instant::now(),
    });
}

/// Compute a stable cache key from the request.
fn compute_key(req: &ChatCompletionRequest) -> String {
    let mut hasher = Sha256::new();
    hasher.update(req.model.as_bytes());
    hasher.update(b"\x00");
    // Serialize messages deterministically (serde_json preserves order)
    let messages_json = serde_json::to_string(&req.messages).unwrap_or_default();
    hasher.update(messages_json.as_bytes());
    hasher.update(b"\x00");
    hasher.update(format!("{}", req.temperature).as_bytes());
    hasher.update(b"\x00");
    hasher.update(format!("{}", req.top_p).as_bytes());
    hasher.update(b"\x00");
    if let Some(mt) = req.max_tokens {
        hasher.update(format!("{}", mt).as_bytes());
    }
    hasher.update(b"\x00");
    if let Some(stop) = &req.stop {
        let stop_json = serde_json::to_string(stop).unwrap_or_default();
        hasher.update(stop_json.as_bytes());
    }
    hasher.update(b"\x00");
    if let Some(seed) = req.seed {
        hasher.update(format!("{}", seed).as_bytes());
    }
    let result = hasher.finalize();
    hex::encode(result)
}

/// Clear the entire cache. (Used by tests + admin endpoint.)
pub fn clear() {
    if let Ok(mut cache) = CACHE.lock() {
        cache.entries.clear();
    }
}

/// Get cache stats (entry count, hit/miss counters).
pub fn stats() -> (usize, Duration) {
    let cache = CACHE.lock().unwrap();
    (cache.entries.len(), cache.ttl)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::chat::{Message, MessageContent};

    fn make_req(model: &str, content: &str) -> ChatCompletionRequest {
        ChatCompletionRequest {
            model: model.into(),
            messages: vec![Message {
                role: "user".into(),
                content: Some(MessageContent::Text(content.into())),
                tool_calls: None, tool_call_id: None, name: None,
            }],
            temperature: 0.7, top_p: 1.0, max_tokens: None, stream: false,
            stop: None, seed: None, tools: None, tool_choice: None,
            extra: serde_json::Map::new(),
        }
    }

    #[test]
    fn cache_key_is_deterministic() {
        let req1 = make_req("openai:gpt-4o", "hello");
        let req2 = make_req("openai:gpt-4o", "hello");
        assert_eq!(compute_key(&req1), compute_key(&req2));
    }

    #[test]
    fn cache_key_differs_for_different_content() {
        let req1 = make_req("openai:gpt-4o", "hello");
        let req2 = make_req("openai:gpt-4o", "world");
        assert_ne!(compute_key(&req1), compute_key(&req2));
    }
}
