//! Hyperbolic — OpenAI-compatible.
//! Endpoint: POST https://api.hyperbolic.xyz/v1/chat/completions

use super::openai::OpenAI;
use crate::models::provider::ProviderId;

const BASE_URL: &str = "https://api.hyperbolic.xyz/v1";
const DEFAULT_MODELS: &[&str] = &[
    "meta-llama/Meta-Llama-3.1-405B-Instruct",
    "meta-llama/Meta-Llama-3.1-70B-Instruct",
    "Qwen/Qwen2.5-72B-Instruct",
    "deepseek-ai/DeepSeek-V3",
];

pub type Hyperbolic = OpenAI;

pub fn new(api_key: Option<String>) -> Hyperbolic {
    OpenAI::with_base_url(api_key, BASE_URL, ProviderId::Hyperbolic, "hyperbolic", DEFAULT_MODELS)
}
