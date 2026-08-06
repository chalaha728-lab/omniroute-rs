//! Mistral AI — OpenAI-compatible.
//! Endpoint: POST https://api.mistral.ai/v1/chat/completions

use super::openai::OpenAI;
use crate::models::provider::ProviderId;

const BASE_URL: &str = "https://api.mistral.ai/v1";
const DEFAULT_MODELS: &[&str] = &[
    "mistral-large-latest",
    "mistral-small-latest",
    "open-mistral-7b",
    "open-mixtral-8x7b",
    "open-mixtral-8x22b",
    "codestral-latest",
    "mistral-embed",
];

pub type Mistral = OpenAI;

pub fn new(api_key: Option<String>) -> Mistral {
    OpenAI::with_base_url(api_key, BASE_URL, ProviderId::Mistral, "mistral", DEFAULT_MODELS)
}
