//! Anyscale — OpenAI-compatible endpoint for fine-tuned Llama models.
//! Endpoint: POST https://api.endpoints.anyscale.com/v1/chat/completions

use super::openai::OpenAI;
use crate::models::provider::ProviderId;

const BASE_URL: &str = "https://api.endpoints.anyscale.com/v1";
const DEFAULT_MODELS: &[&str] = &[
    "meta-llama/Llama-3.1-8B-Instruct",
    "meta-llama/Llama-3.1-70B-Instruct",
    "meta-llama/Meta-Llama-3-70B-Instruct",
    "mistralai/Mixtral-8x7B-Instruct-v0.1",
];

pub type Anyscale = OpenAI;

pub fn new(api_key: Option<String>) -> Anyscale {
    OpenAI::with_base_url(api_key, BASE_URL, ProviderId::Anyscale, "anyscale", DEFAULT_MODELS)
}
