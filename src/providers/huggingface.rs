//! Hugging Face Inference API (OpenAI-compatible chat completions).
//! Endpoint: POST https://api-inference.huggingface.co/v1/chat/completions

use super::openai::OpenAI;
use crate::models::provider::ProviderId;

const BASE_URL: &str = "https://api-inference.huggingface.co/v1";
const DEFAULT_MODELS: &[&str] = &[
    "meta-llama/Llama-3.3-70B-Instruct",
    "meta-llama/Llama-3.1-8B-Instruct",
    "mistralai/Mistral-7B-Instruct-v0.3",
    "Qwen/Qwen2.5-72B-Instruct",
];

pub type HuggingFace = OpenAI;

pub fn new(api_key: Option<String>) -> HuggingFace {
    OpenAI::with_base_url(api_key, BASE_URL, ProviderId::HuggingFace, "huggingface", DEFAULT_MODELS)
}
