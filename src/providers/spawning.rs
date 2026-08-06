//! Spawning AI — OpenAI-compatible.
//! Endpoint: POST https://api.spawningai.app/v1/chat/completions

use super::openai::OpenAI;
use crate::models::provider::ProviderId;

const BASE_URL: &str = "https://api.spawningai.app/v1";
const DEFAULT_MODELS: &[&str] = &[
    "spawning/spawning-7b",
    "meta-llama/Meta-Llama-3.1-8B-Instruct",
];

pub type Spawning = OpenAI;

pub fn new(api_key: Option<String>) -> Spawning {
    OpenAI::with_base_url(api_key, BASE_URL, ProviderId::Spawning, "spawning", DEFAULT_MODELS)
}
