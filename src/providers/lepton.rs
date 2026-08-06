//! Lepton AI — OpenAI-compatible.
//! Endpoint: POST https://{provider}.lepton.run/api/v1/chat/completions
//! Default: https://api.lepton.run/api/v1

use super::openai::OpenAI;
use crate::models::provider::ProviderId;

const BASE_URL: &str = "https://api.lepton.run/api/v1";
const DEFAULT_MODELS: &[&str] = &[
    "gpt-4o-mini",
    "gpt-4o",
    "claude-3-5-sonnet",
    "llama3.1-8b",
    "llama3.1-405b",
];

pub type LeptonAI = OpenAI;

pub fn new(api_key: Option<String>) -> LeptonAI {
    OpenAI::with_base_url(api_key, BASE_URL, ProviderId::LeptonAI, "lepton", DEFAULT_MODELS)
}
