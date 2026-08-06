//! Fireworks AI — OpenAI-compatible.
//! Endpoint: POST https://api.fireworks.ai/inference/v1/chat/completions

use super::openai::OpenAI;
use crate::models::provider::ProviderId;

const BASE_URL: &str = "https://api.fireworks.ai/inference/v1";
const DEFAULT_MODELS: &[&str] = &[
    "accounts/fireworks/models/llama-v3p3-70b-instruct",
    "accounts/fireworks/models/llama-v3p1-8b-instruct",
    "accounts/fireworks/models/mixtral-8x7b-instruct",
    "accounts/fireworks/models/qwen2p5-72b-instruct",
];

pub type Fireworks = OpenAI;

pub fn new(api_key: Option<String>) -> Fireworks {
    OpenAI::with_base_url(api_key, BASE_URL, ProviderId::Fireworks, "fireworks", DEFAULT_MODELS)
}
