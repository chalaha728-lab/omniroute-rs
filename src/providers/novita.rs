//! Novita AI — OpenAI-compatible.
//! Endpoint: POST https://api.novita.ai/v3/openai/chat/completions

use super::openai::OpenAI;
use crate::models::provider::ProviderId;

const BASE_URL: &str = "https://api.novita.ai/v3/openai";
const DEFAULT_MODELS: &[&str] = &[
    "meta-llama/llama-3.3-70b-instruct",
    "meta-llama/llama-3.1-8b-instruct",
    "deepseek/deepseek-v3",
    "qwen/qwen2.5-72b-instruct",
];

pub type Novita = OpenAI;

pub fn new(api_key: Option<String>) -> Novita {
    OpenAI::with_base_url(api_key, BASE_URL, ProviderId::Novita, "novita", DEFAULT_MODELS)
}
