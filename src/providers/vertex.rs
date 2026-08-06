//! Google Vertex AI — OpenAI-compatible endpoint.
//! Endpoint: POST https://{location}-aiplatform.googleapis.com/v1/projects/{project}/locations/{location}/endpoints/openapi/chat/completions
//!
//! Auth: Bearer token (Google OAuth2 access token — short-lived, ~1 hour).
//!       Set VERTEX_PROJECT, VERTEX_LOCATION, VERTEX_ACCESS_TOKEN env vars.
//!
//! For simplicity, this impl uses the OpenAI-compatible wrapper. For full
//! native Vertex support (with automatic token refresh from a service account
//! JSON key), see the Google Cloud auth crate.

use super::openai::OpenAI;
use crate::models::provider::ProviderId;

fn build_base_url() -> String {
    let project = std::env::var("VERTEX_PROJECT").unwrap_or_else(|_| "your-project".into());
    let location = std::env::var("VERTEX_LOCATION").unwrap_or_else(|_| "us-central1".into());
    format!(
        "https://{}-aiplatform.googleapis.com/v1/projects/{}/locations/{}/endpoints/openapi",
        location, project, location
    )
}

const DEFAULT_MODELS: &[&str] = &[
    "google/gemini-1.5-pro",
    "google/gemini-1.5-flash",
    "google/gemini-2.0-flash-exp",
    "meta/llama-3.1-405b-instruct",
    "meta/llama-3.1-70b-instruct",
    "anthropic/claude-3-5-sonnet",
    "anthropic/claude-3-5-haiku",
];

pub type Vertex = OpenAI;

pub fn new(access_token: Option<String>) -> Vertex {
    OpenAI::with_base_url(access_token, build_base_url(), ProviderId::Vertex, "google", DEFAULT_MODELS)
}
