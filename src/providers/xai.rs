//! xAI (Grok) — OpenAI-compatible.
//! Endpoint: POST https://api.x.ai/v1/chat/completions

use super::openai::OpenAI;
use crate::models::provider::ProviderId;

const BASE_URL: &str = "https://api.x.ai/v1";
const DEFAULT_MODELS: &[&str] = &["grok-2-latest", "grok-2-1212", "grok-beta", "grok-vision-beta"];

pub type XAI = OpenAI;

pub fn new(api_key: Option<String>) -> XAI {
    OpenAI::with_base_url(api_key, BASE_URL, ProviderId::XAI, "xai", DEFAULT_MODELS)
}
