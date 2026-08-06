//! Groq — ultra-low-latency OpenAI-compatible inference.
//! Endpoint: POST https://api.groq.com/openai/v1/chat/completions

use super::openai::OpenAI;
use crate::models::provider::ProviderId;

const BASE_URL: &str = "https://api.groq.com/openai/v1";
const DEFAULT_MODELS: &[&str] = &[
    "llama-3.3-70b-versatile",
    "llama-3.1-8b-instant",
    "mixtral-8x7b-32768",
    "gemma2-9b-it",
];

pub type Groq = OpenAI;

pub fn new(api_key: Option<String>) -> Groq {
    OpenAI::with_base_url(api_key, BASE_URL, ProviderId::Groq, "groq", DEFAULT_MODELS)
}
