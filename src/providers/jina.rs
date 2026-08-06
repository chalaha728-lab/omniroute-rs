//! Jina AI — embeddings + reranker + reader. OpenAI-compatible.
//! Endpoint: POST https://api.jina.ai/v1/chat/completions

use super::openai::OpenAI;
use crate::models::provider::ProviderId;

const BASE_URL: &str = "https://api.jina.ai/v1";
const DEFAULT_MODELS: &[&str] = &[
    "jina-embeddings-v3",
    "jina-embeddings-v2-base-en",
    "jina-embeddings-v2-base-code",
    "jina-reranker-v2-base-multilingual",
    "jina-reader-v1",
];

pub type Jina = OpenAI;

pub fn new(api_key: Option<String>) -> Jina {
    OpenAI::with_base_url(api_key, BASE_URL, ProviderId::Jina, "jina", DEFAULT_MODELS)
}
