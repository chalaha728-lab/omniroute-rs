//! OpenRouter provider — OpenAI-compatible aggregator.
//!
//! Endpoint: POST https://openrouter.ai/api/v1/chat/completions
//! Docs: https://openrouter.ai/docs
//! Reuses the OpenAI implementation since OpenRouter is wire-compatible.

use super::openai::OpenAI;
use crate::models::provider::ProviderId;

const BASE_URL: &str = "https://openrouter.ai/api/v1";
const DEFAULT_MODELS: &[&str] = &[
    "openai/gpt-4o",
    "openai/gpt-4o-mini",
    "anthropic/claude-3.5-sonnet",
    "anthropic/claude-3.5-haiku",
    "google/gemini-flash-1.5",
    "google/gemini-pro-1.5",
    "meta-llama/llama-3.1-70b-instruct",
    "deepseek/deepseek-chat",
];

pub type OpenRouter = OpenAI;

pub fn new(api_key: Option<String>) -> OpenRouter {
    OpenAI::with_base_url(
        api_key,
        BASE_URL,
        ProviderId::OpenRouter,
        "openrouter",
        DEFAULT_MODELS,
    )
}
