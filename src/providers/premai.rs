//! PremAI — OpenAI-compatible.
//! Endpoint: POST https://api.premai.io/v1/chat/completions

use super::openai::OpenAI;
use crate::models::provider::ProviderId;

const BASE_URL: &str = "https://api.premai.io/v1";
const DEFAULT_MODELS: &[&str] = &[
    "premai/Premai-Coder-7B",
    "meta-llama/Meta-Llama-3.1-8B-Instruct",
    "meta-llama/Meta-Llama-3.1-70B-Instruct",
];

pub type PremAI = OpenAI;

pub fn new(api_key: Option<String>) -> PremAI {
    OpenAI::with_base_url(api_key, BASE_URL, ProviderId::PremAI, "premai", DEFAULT_MODELS)
}
