//! DeepInfra — OpenAI-compatible.
//! Endpoint: POST https://api.deepinfra.com/v1/openai/chat/completions

use super::openai::OpenAI;
use crate::models::provider::ProviderId;

const BASE_URL: &str = "https://api.deepinfra.com/v1/openai";
const DEFAULT_MODELS: &[&str] = &[
    "meta-llama/Llama-3.3-70B-Instruct",
    "meta-llama/Meta-Llama-3.1-8B-Instruct",
    "deepseek-ai/DeepSeek-V3",
    "Qwen/Qwen2.5-72B-Instruct",
];

pub type DeepInfra = OpenAI;

pub fn new(api_key: Option<String>) -> DeepInfra {
    OpenAI::with_base_url(api_key, BASE_URL, ProviderId::DeepInfra, "deepinfra", DEFAULT_MODELS)
}
