//! Baseten — OpenAI-compatible.
//! Endpoint: POST https://api.baseten.co/v1/chat/completions

use super::openai::OpenAI;
use crate::models::provider::ProviderId;

const BASE_URL: &str = "https://api.baseten.co/v1";
const DEFAULT_MODELS: &[&str] = &[
    "meta-llama/Llama-3.3-70B-Instruct",
    "meta-llama/Meta-Llama-3.1-8B-Instruct",
    "mistralai/Mixtral-8x7B-Instruct-v0.1",
    "deepseek-ai/DeepSeek-V3",
];

pub type Baseten = OpenAI;

pub fn new(api_key: Option<String>) -> Baseten {
    OpenAI::with_base_url(api_key, BASE_URL, ProviderId::Baseten, "baseten", DEFAULT_MODELS)
}
