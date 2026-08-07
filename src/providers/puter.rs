//! Puter.js — free LLM access via the Puter cloud, no API key required.
//! Endpoint: POST https://api.puter.com/puterai/chat/completions
//! Docs: https://docs.puter.com/AI/chat/
//!
//! Puter offers free access to GPT-4o, Claude, etc. through their cloud.
//! No signup needed for basic usage.

use super::openai::OpenAI;
use crate::models::provider::ProviderId;

const BASE_URL: &str = "https://api.puter.com/puterai";
const DEFAULT_MODELS: &[&str] = &[
    "gpt-4o",
    "gpt-4o-mini",
    "claude-3-5-sonnet",
    "claude-3-5-haiku",
    "gemini-1.5-flash",
    "gemini-1.5-pro",
    "o1-mini",
    "deepseek-chat",
];

pub type Puter = OpenAI;

pub fn new() -> Puter {
    // No API key needed — pass a dummy key to satisfy is_configured()
    OpenAI::with_base_url(
        Some("puter-free".into()),
        BASE_URL,
        ProviderId::Puter,
        "puter",
        DEFAULT_MODELS,
    )
}
