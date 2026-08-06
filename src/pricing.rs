//! Per-provider cost calculation — pricing table + auto-compute `cost_usd` per request.
//!
//! Pricing is stored in USD per million tokens (prompt + completion separately).
//! Falls back to 0 if a model isn't in the table (no error — just unknown cost).
//!
//! Prices are sourced from each provider's public pricing page. Update quarterly.
//!
//! Routes:
//!   GET  /api/dashboard/pricing         — full pricing table
//!   GET  /api/dashboard/pricing/:provider — single provider's models
//!   PUT  /api/dashboard/pricing/:provider/:model — override a price (admin)

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;

use crate::models::provider::ProviderId;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Price {
    /// USD per 1 million prompt tokens
    pub prompt_per_mtok: f64,
    /// USD per 1 million completion tokens
    pub completion_per_mtok: f64,
}

impl Price {
    /// Compute the cost (USD) for a request with the given token counts.
    pub fn cost(&self, prompt_tokens: u32, completion_tokens: u32) -> f64 {
        (prompt_tokens as f64 * self.prompt_per_mtok / 1_000_000.0)
            + (completion_tokens as f64 * self.completion_per_mtok / 1_000_000.0)
    }
}

/// Lookup key: "<provider>:<model>" (lowercase, normalized).
type PriceTable = HashMap<String, Price>;

/// Default pricing — sourced from each provider's public pricing page (Q4 2025).
/// Override at runtime via the dashboard API.
static DEFAULT_PRICES: Lazy<HashMap<&'static str, Price>> = Lazy::new(|| {
    let mut m: HashMap<&'static str, Price> = HashMap::new();
    // OpenAI
    m.insert("openai:gpt-4o", Price { prompt_per_mtok: 2.50, completion_per_mtok: 10.00 });
    m.insert("openai:gpt-4o-mini", Price { prompt_per_mtok: 0.15, completion_per_mtok: 0.60 });
    m.insert("openai:gpt-4-turbo", Price { prompt_per_mtok: 10.00, completion_per_mtok: 30.00 });
    m.insert("openai:gpt-4", Price { prompt_per_mtok: 30.00, completion_per_mtok: 60.00 });
    m.insert("openai:gpt-3.5-turbo", Price { prompt_per_mtok: 0.50, completion_per_mtok: 1.50 });
    m.insert("openai:o1", Price { prompt_per_mtok: 15.00, completion_per_mtok: 60.00 });
    m.insert("openai:o1-mini", Price { prompt_per_mtok: 3.00, completion_per_mtok: 12.00 });
    m.insert("openai:o1-preview", Price { prompt_per_mtok: 15.00, completion_per_mtok: 60.00 });
    m.insert("openai:o3-mini", Price { prompt_per_mtok: 3.00, completion_per_mtok: 12.00 });
    m.insert("openai:text-embedding-3-small", Price { prompt_per_mtok: 0.02, completion_per_mtok: 0.0 });
    m.insert("openai:text-embedding-3-large", Price { prompt_per_mtok: 0.13, completion_per_mtok: 0.0 });
    m.insert("openai:dall-e-3", Price { prompt_per_mtok: 40.00, completion_per_mtok: 0.0 }); // per-image equiv
    m.insert("openai:tts-1", Price { prompt_per_mtok: 15.00, completion_per_mtok: 0.0 });
    m.insert("openai:tts-1-hd", Price { prompt_per_mtok: 30.00, completion_per_mtok: 0.0 });
    // Anthropic
    m.insert("anthropic:claude-3-5-sonnet-20241022", Price { prompt_per_mtok: 3.00, completion_per_mtok: 15.00 });
    m.insert("anthropic:claude-3-5-haiku-20241022", Price { prompt_per_mtok: 0.80, completion_per_mtok: 4.00 });
    m.insert("anthropic:claude-3-opus-20240229", Price { prompt_per_mtok: 15.00, completion_per_mtok: 75.00 });
    m.insert("anthropic:claude-3-sonnet-20240229", Price { prompt_per_mtok: 3.00, completion_per_mtok: 15.00 });
    m.insert("anthropic:claude-3-haiku-20240307", Price { prompt_per_mtok: 0.25, completion_per_mtok: 1.25 });
    // Google Gemini
    m.insert("gemini:gemini-1.5-pro", Price { prompt_per_mtok: 1.25, completion_per_mtok: 5.00 });
    m.insert("gemini:gemini-1.5-flash", Price { prompt_per_mtok: 0.075, completion_per_mtok: 0.30 });
    m.insert("gemini:gemini-1.5-flash-8b", Price { prompt_per_mtok: 0.0375, completion_per_mtok: 0.15 });
    m.insert("gemini:gemini-2.0-flash-exp", Price { prompt_per_mtok: 0.075, completion_per_mtok: 0.30 });
    // DeepSeek
    m.insert("deepseek:deepseek-chat", Price { prompt_per_mtok: 0.14, completion_per_mtok: 0.28 });
    m.insert("deepseek:deepseek-reasoner", Price { prompt_per_mtok: 0.55, completion_per_mtok: 2.19 });
    // OpenRouter (varies — these are cache hits; non-cache is higher)
    m.insert("openrouter:openai/gpt-4o", Price { prompt_per_mtok: 2.50, completion_per_mtok: 10.00 });
    m.insert("openrouter:anthropic/claude-3.5-sonnet", Price { prompt_per_mtok: 3.00, completion_per_mtok: 15.00 });
    m.insert("openrouter:google/gemini-flash-1.5", Price { prompt_per_mtok: 0.075, completion_per_mtok: 0.30 });
    // Groq (very cheap — Meta-hosted)
    m.insert("groq:llama-3.3-70b-versatile", Price { prompt_per_mtok: 0.59, completion_per_mtok: 0.79 });
    m.insert("groq:llama-3.1-8b-instant", Price { prompt_per_mtok: 0.05, completion_per_mtok: 0.08 });
    m.insert("groq:mixtral-8x7b-32768", Price { prompt_per_mtok: 0.24, completion_per_mtok: 0.24 });
    // Mistral
    m.insert("mistral:mistral-large-latest", Price { prompt_per_mtok: 2.00, completion_per_mtok: 6.00 });
    m.insert("mistral:mistral-small-latest", Price { prompt_per_mtok: 0.20, completion_per_mtok: 0.60 });
    m.insert("mistral:codestral-latest", Price { prompt_per_mtok: 0.30, completion_per_mtok: 0.90 });
    // xAI
    m.insert("xai:grok-2-latest", Price { prompt_per_mtok: 2.00, completion_per_mtok: 10.00 });
    m.insert("xai:grok-beta", Price { prompt_per_mtok: 5.00, completion_per_mtok: 15.00 });
    // Perplexity
    m.insert("perplexity:sonar-pro", Price { prompt_per_mtok: 3.00, completion_per_mtok: 15.00 });
    m.insert("perplexity:sonar", Price { prompt_per_mtok: 1.00, completion_per_mtok: 1.00 });
    m.insert("perplexity:sonar-reasoning", Price { prompt_per_mtok: 2.00, completion_per_mtok: 8.00 });
    m.insert("perplexity:sonar-deep-research", Price { prompt_per_mtok: 2.00, completion_per_mtok: 8.00 });
    // Cohere
    m.insert("cohere:command-r-plus-08-2024", Price { prompt_per_mtok: 2.50, completion_per_mtok: 10.00 });
    m.insert("cohere:command-r-08-2024", Price { prompt_per_mtok: 0.15, completion_per_mtok: 0.60 });
    // Together AI (Llama models — pass-through pricing)
    m.insert("together:meta-llama/Llama-3.3-70B-Instruct-Turbo", Price { prompt_per_mtok: 0.88, completion_per_mtok: 0.88 });
    m.insert("together:meta-llama/Meta-Llama-3.1-8B-Instruct-Turbo", Price { prompt_per_mtok: 0.18, completion_per_mtok: 0.18 });
    // Fireworks
    m.insert("fireworks:accounts/fireworks/models/llama-v3p3-70b-instruct", Price { prompt_per_mtok: 0.90, completion_per_mtok: 0.90 });
    // Voyage (embeddings)
    m.insert("voyage:voyage-3-large", Price { prompt_per_mtok: 0.18, completion_per_mtok: 0.0 });
    m.insert("voyage:voyage-3", Price { prompt_per_mtok: 0.06, completion_per_mtok: 0.0 });
    m.insert("voyage:voyage-3-lite", Price { prompt_per_mtok: 0.02, completion_per_mtok: 0.0 });
    // Jina (embeddings)
    m.insert("jina:jina-embeddings-v3", Price { prompt_per_mtok: 0.18, completion_per_mtok: 0.0 });
    m
});

/// Runtime-overridable price table (admin can override defaults via dashboard API).
static OVERRIDES: Lazy<RwLock<PriceTable>> = Lazy::new(|| RwLock::new(HashMap::new()));

/// Lookup the price for a model. Returns None if unknown.
pub fn lookup(provider: ProviderId, model: &str) -> Option<Price> {
    let key = format!("{}:{}", provider.as_str(), model.to_lowercase());
    // Check overrides first
    if let Some(p) = OVERRIDES.read().ok()?.get(&key) {
        return Some(*p);
    }
    // Then defaults
    DEFAULT_PRICES.get(key.as_str()).copied()
}

/// Compute the cost (USD) for a request.
pub fn compute_cost(provider: ProviderId, model: &str, prompt_tokens: u32, completion_tokens: u32) -> f64 {
    lookup(provider, model)
        .map(|p| p.cost(prompt_tokens, completion_tokens))
        .unwrap_or(0.0)
}

/// List all known prices (defaults + overrides).
pub fn list_all() -> Vec<(String, Price)> {
    let mut out: Vec<(String, Price)> = DEFAULT_PRICES.iter()
        .map(|(k, v)| (k.to_string(), *v))
        .collect();
    if let Ok(overrides) = OVERRIDES.read() {
        for (k, v) in overrides.iter() {
            // Override replaces default if same key
            if let Some(idx) = out.iter().position(|(dk, _)| dk == k) {
                out[idx].1 = *v;
            } else {
                out.push((k.clone(), *v));
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// Set an override price for a specific model.
pub fn set_override(provider: ProviderId, model: &str, price: Price) {
    let key = format!("{}:{}", provider.as_str(), model.to_lowercase());
    if let Ok(mut overrides) = OVERRIDES.write() {
        overrides.insert(key, price);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_model_returns_price() {
        let p = lookup(ProviderId::OpenAI, "gpt-4o").expect("gpt-4o should have a price");
        assert!(p.prompt_per_mtok > 0.0);
        assert!(p.completion_per_mtok > 0.0);
    }

    #[test]
    fn unknown_model_returns_none() {
        assert!(lookup(ProviderId::OpenAI, "nonexistent-model").is_none());
    }

    #[test]
    fn cost_computation() {
        // gpt-4o: $2.50/M prompt, $10.00/M completion
        // 1M prompt + 1M completion = $12.50
        let cost = compute_cost(ProviderId::OpenAI, "gpt-4o", 1_000_000, 1_000_000);
        assert!((cost - 12.50).abs() < 0.01);
    }

    #[test]
    fn override_takes_precedence() {
        set_override(ProviderId::OpenAI, "gpt-4o", Price { prompt_per_mtok: 99.99, completion_per_mtok: 99.99 });
        let p = lookup(ProviderId::OpenAI, "gpt-4o").unwrap();
        assert!((p.prompt_per_mtok - 99.99).abs() < 0.01);
    }
}
