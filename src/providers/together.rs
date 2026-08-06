//! Together AI — OpenAI-compatible.
//! Endpoint: POST https://api.together.xyz/v1/chat/completions

use super::openai::OpenAI;
use crate::models::provider::ProviderId;

const BASE_URL: &str = "https://api.together.xyz/v1";
const DEFAULT_MODELS: &[&str] = &[
    "meta-llama/Llama-3.3-70B-Instruct-Turbo",
    "meta-llama/Meta-Llama-3.1-8B-Instruct-Turbo",
    "meta-llama/Meta-Llama-3.1-70B-Instruct-Turbo",
    "mistralai/Mistral-7B-Instruct-v0.3",
    "Qwen/Qwen2.5-72B-Instruct-Turbo",
    "deepseek-ai/DeepSeek-V3",
];

pub type Together = OpenAI;

pub fn new(api_key: Option<String>) -> Together {
    OpenAI::with_base_url(api_key, BASE_URL, ProviderId::Together, "together", DEFAULT_MODELS)
}
