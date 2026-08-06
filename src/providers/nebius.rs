//! Nebius AI — OpenAI-compatible.
//! Endpoint: POST https://api.studio.nebius.com/v1/chat/completions

use super::openai::OpenAI;
use crate::models::provider::ProviderId;

const BASE_URL: &str = "https://api.studio.nebius.com/v1";
const DEFAULT_MODELS: &[&str] = &[
    "meta-llama/Meta-Llama-3.1-405B-Instruct",
    "meta-llama/Meta-Llama-3.1-70B-Instruct",
    "Qwen/Qwen2.5-32B-Instruct",
    "deepseek-ai/DeepSeek-V3",
];

pub type Nebius = OpenAI;

pub fn new(api_key: Option<String>) -> Nebius {
    OpenAI::with_base_url(api_key, BASE_URL, ProviderId::Nebius, "nebius", DEFAULT_MODELS)
}
