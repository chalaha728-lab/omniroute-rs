//! Ollama — local LLM runtime. OpenAI-compatible.
//! Endpoint: POST http://localhost:11434/v1/chat/completions
//! Auth: none required (local). Override URL via OLLAMA_BASE_URL env var.

use super::openai::OpenAI;
use crate::models::provider::ProviderId;

const DEFAULT_BASE_URL: &str = "http://localhost:11434/v1";
const DEFAULT_MODELS: &[&str] = &[
    "llama3.3",
    "llama3.1",
    "llama3.2",
    "qwen2.5",
    "mistral",
    "deepseek-r1",
    "phi-4",
    "gemma2",
];

pub type Ollama = OpenAI;

pub fn new(base_url: Option<String>) -> Ollama {
    let url = base_url.filter(|s| !s.is_empty())
        .map(|s| {
            // Ensure the URL ends in /v1
            let s = s.trim_end_matches('/');
            if s.ends_with("/v1") { s.to_string() } else { format!("{}/v1", s) }
        })
        .unwrap_or_else(|| DEFAULT_BASE_URL.into());
    // Ollama doesn't need a key, but our OpenAI impl requires one to be "configured".
    // Pass a dummy key to satisfy is_configured().
    OpenAI::with_base_url(Some("ollama".into()), url, ProviderId::Ollama, "ollama", DEFAULT_MODELS)
}
