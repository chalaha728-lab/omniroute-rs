//! Scaleway — OpenAI-compatible (hosted in EU/FR).
//! Endpoint: POST https://api.scaleway.ai/ai-models/v1/chat/completions

use super::openai::OpenAI;
use crate::models::provider::ProviderId;

const BASE_URL: &str = "https://api.scaleway.ai/ai-models/v1";
const DEFAULT_MODELS: &[&str] = &[
    "llama-3.1-8b-instruct",
    "llama-3.1-70b-instruct",
    "mistral-7b-instruct-v0.3",
    "qwen2.5-72b-instruct",
];

pub type Scaleway = OpenAI;

pub fn new(api_key: Option<String>) -> Scaleway {
    OpenAI::with_base_url(api_key, BASE_URL, ProviderId::Scaleway, "scaleway", DEFAULT_MODELS)
}
