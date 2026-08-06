//! Replicate — uses predictions API (not OpenAI-compatible), but we expose the
//! chat-compatible endpoint via the OpenAI-compatible shim at /v1/chat/completions
//! when a chat model is requested. For now we point at the OpenAI-compatible
//! endpoint hosted at predictions.replicate.com.
//!
//! Endpoint: POST https://api.replicate.com/v1/chat/completions
//! Auth: Bearer token (REPLICATE_API_TOKEN)

use super::openai::OpenAI;
use crate::models::provider::ProviderId;

const BASE_URL: &str = "https://api.replicate.com/v1";
const DEFAULT_MODELS: &[&str] = &[
    "meta/llama-3.3-70b-instruct",
    "meta/llama-3.1-8b-instruct",
    "mistralai/mixtral-8x7b-instruct-v0.1",
    "cognitivecomputations/dolphin-mixtral-8x7b",
];

pub type Replicate = OpenAI;

pub fn new(api_key: Option<String>) -> Replicate {
    OpenAI::with_base_url(api_key, BASE_URL, ProviderId::Replicate, "replicate", DEFAULT_MODELS)
}
