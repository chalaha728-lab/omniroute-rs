//! OctoAI — OpenAI-compatible.
//! Endpoint: POST https://text.octoai.run/v1/chat/completions

use super::openai::OpenAI;
use crate::models::provider::ProviderId;

const BASE_URL: &str = "https://text.octoai.run/v1";
const DEFAULT_MODELS: &[&str] = &[
    "meta-llama-3.1-70b-instruct",
    "meta-llama-3.1-8b-instruct",
    "mistral-7b-instruct-v0.3",
];

pub type OctoAI = OpenAI;

pub fn new(api_key: Option<String>) -> OctoAI {
    OpenAI::with_base_url(api_key, BASE_URL, ProviderId::OctoAI, "octoai", DEFAULT_MODELS)
}
