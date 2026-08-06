//! Cerebras — OpenAI-compatible, ultra-fast inference.
//! Endpoint: POST https://api.cerebras.ai/v1/chat/completions

use super::openai::OpenAI;
use crate::models::provider::ProviderId;

const BASE_URL: &str = "https://api.cerebras.ai/v1";
const DEFAULT_MODELS: &[&str] = &[
    "llama-3.3-70b",
    "llama3.1-8b",
    "qwen-3-32b",
];

pub type Cerebras = OpenAI;

pub fn new(api_key: Option<String>) -> Cerebras {
    OpenAI::with_base_url(api_key, BASE_URL, ProviderId::Cerebras, "cerebras", DEFAULT_MODELS)
}
