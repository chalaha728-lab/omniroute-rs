//! FriendliAI — OpenAI-compatible inference (Llama, Mixtral, etc.).
//! Endpoint: POST https://api.friendli.ai/api/v1/chat/completions

use super::openai::OpenAI;
use crate::models::provider::ProviderId;

const BASE_URL: &str = "https://api.friendli.ai/api/v1";
const DEFAULT_MODELS: &[&str] = &[
    "meta-llama-3.1-8b-instruct",
    "meta-llama-3.1-70b-instruct",
    "mistral-7b-instruct-v0.2",
    "mixtral-8x7b-instruct-v0.1",
];

pub type FriendliAI = OpenAI;

pub fn new(api_key: Option<String>) -> FriendliAI {
    OpenAI::with_base_url(api_key, BASE_URL, ProviderId::FriendliAI, "friendli", DEFAULT_MODELS)
}
