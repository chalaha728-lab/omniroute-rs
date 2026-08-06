//! DeepSeek provider — OpenAI-compatible API.
//!
//! Endpoint: POST https://api.deepseek.com/v1/chat/completions
//! Docs: https://api-docs.deepseek.com/
//! Reuses the OpenAI implementation since DeepSeek is wire-compatible.

use super::openai::OpenAI;
use crate::models::provider::ProviderId;

const BASE_URL: &str = "https://api.deepseek.com/v1";
const DEFAULT_MODELS: &[&str] = &["deepseek-chat", "deepseek-reasoner"];

pub type DeepSeek = OpenAI;

pub fn new(api_key: Option<String>) -> DeepSeek {
    OpenAI::with_base_url(
        api_key,
        BASE_URL,
        ProviderId::DeepSeek,
        "deepseek",
        DEFAULT_MODELS,
    )
}
