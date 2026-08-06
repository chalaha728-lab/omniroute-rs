//! SambaNova — OpenAI-compatible.
//! Endpoint: POST https://api.sambanova.ai/v1/chat/completions

use super::openai::OpenAI;
use crate::models::provider::ProviderId;

const BASE_URL: &str = "https://api.sambanova.ai/v1";
const DEFAULT_MODELS: &[&str] = &[
    "Meta-Llama-3.3-70B-Instruct",
    "Meta-Llama-3.1-405B-Instruct",
    "Meta-Llama-3.1-8B-Instruct",
    "DeepSeek-V3",
];

pub type SambaNova = OpenAI;

pub fn new(api_key: Option<String>) -> SambaNova {
    OpenAI::with_base_url(api_key, BASE_URL, ProviderId::SambaNova, "sambanova", DEFAULT_MODELS)
}
