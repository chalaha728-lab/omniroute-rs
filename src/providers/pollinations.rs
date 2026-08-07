//! Pollinations.ai — FREE text generation, no API key required.
//! Endpoint: POST https://text.pollinations.ai/openai
//! Docs: https://pollinations.ai/
//!
//! This is a genuinely free provider — no signup, no API key, no rate limit
//! (beyond fair use). Perfect for testing the gateway without any credentials.

use super::openai::OpenAI;
use crate::models::provider::ProviderId;

const BASE_URL: &str = "https://text.pollinations.ai/openai";
const DEFAULT_MODELS: &[&str] = &[
    "openai",
    "openai-large",
    "openai-reasoning",
    "qwen-coder",
    "llama",
    "mistral",
    "deepseek",
    "searchgpt",
];

pub type Pollinations = OpenAI;

pub fn new() -> Pollinations {
    // No API key needed — pass a dummy key to satisfy is_configured()
    OpenAI::with_base_url(
        Some("pollinations-free".into()),
        BASE_URL,
        ProviderId::Pollinations,
        "pollinations",
        DEFAULT_MODELS,
    )
}
