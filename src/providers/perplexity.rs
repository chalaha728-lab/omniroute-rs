//! Perplexity — OpenAI-compatible.
//! Endpoint: POST https://api.perplexity.ai/chat/completions

use super::openai::OpenAI;
use crate::models::provider::ProviderId;

const BASE_URL: &str = "https://api.perplexity.ai";
const DEFAULT_MODELS: &[&str] = &[
    "sonar-pro",
    "sonar",
    "sonar-reasoning",
    "sonar-reasoning-pro",
    "sonar-deep-research",
];

pub type Perplexity = OpenAI;

pub fn new(api_key: Option<String>) -> Perplexity {
    OpenAI::with_base_url(api_key, BASE_URL, ProviderId::Perplexity, "perplexity", DEFAULT_MODELS)
}
