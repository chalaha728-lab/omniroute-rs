//! Voyage AI — embeddings specialist. OpenAI-compatible.
//! Endpoint: POST https://api.voyageai.com/v1/chat/completions (chat)
//!           POST https://api.voyageai.com/v1/embeddings (embeddings)
//!
//! Voyage is primarily an embeddings provider, but offers a chat completions
//! endpoint that routes to partner models.

use super::openai::OpenAI;
use crate::models::provider::ProviderId;

const BASE_URL: &str = "https://api.voyageai.com/v1";
const DEFAULT_MODELS: &[&str] = &[
    "voyage-3-large",
    "voyage-3",
    "voyage-3-lite",
    "voyage-code-3",
    "voyage-finance-2",
    "voyage-law-2",
];

pub type Voyage = OpenAI;

pub fn new(api_key: Option<String>) -> Voyage {
    OpenAI::with_base_url(api_key, BASE_URL, ProviderId::Voyage, "voyage", DEFAULT_MODELS)
}
