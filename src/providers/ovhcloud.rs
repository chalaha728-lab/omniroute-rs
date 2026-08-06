//! OVHcloud — OpenAI-compatible (EU-hosted).
//! Endpoint: POST https://endpoints.ai.cloud.ovh.net/v1/chat/completions

use super::openai::OpenAI;
use crate::models::provider::ProviderId;

const BASE_URL: &str = "https://endpoints.ai.cloud.ovh.net/v1";
const DEFAULT_MODELS: &[&str] = &[
    "Meta-Llama-3.1-70B-Instruct",
    "Meta-Llama-3.1-8B-Instruct",
    "Mistral-7B-Instruct-v0.3",
    "Mixtral-8x7B-Instruct-v0.1",
    "qwen2.5-72b-instruct",
];

pub type OVHcloud = OpenAI;

pub fn new(api_key: Option<String>) -> OVHcloud {
    OpenAI::with_base_url(api_key, BASE_URL, ProviderId::OVHcloud, "ovh", DEFAULT_MODELS)
}
