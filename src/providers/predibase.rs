//! Predibase — OpenAI-compatible fine-tuned LoRA serving.
//! Endpoint: POST https://serving.app.predibase.com/v1/chat/completions

use super::openai::OpenAI;
use crate::models::provider::ProviderId;

const BASE_URL: &str = "https://serving.app.predibase.com/v1";
const DEFAULT_MODELS: &[&str] = &[
    "meta-llama/Meta-Llama-3.1-8B-Instruct",
    "meta-llama/Meta-Llama-3.1-70B-Instruct",
    "mistralai/Mistral-7B-Instruct-v0.3",
];

pub type Predibase = OpenAI;

pub fn new(api_key: Option<String>) -> Predibase {
    OpenAI::with_base_url(api_key, BASE_URL, ProviderId::Predibase, "predibase", DEFAULT_MODELS)
}
