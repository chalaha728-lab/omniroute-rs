//! AI21 Labs — OpenAI-compatible.
//! Endpoint: POST https://api.ai21.com/studio/v1/chat/completions

use super::openai::OpenAI;
use crate::models::provider::ProviderId;

const BASE_URL: &str = "https://api.ai21.com/studio/v1";
const DEFAULT_MODELS: &[&str] = &["jamba-1.5-large", "jamba-1.5-mini"];

pub type AI21 = OpenAI;

pub fn new(api_key: Option<String>) -> AI21 {
    OpenAI::with_base_url(api_key, BASE_URL, ProviderId::AI21, "ai21", DEFAULT_MODELS)
}
