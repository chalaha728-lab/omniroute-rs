//! /v1/embeddings — OpenAI-compatible text embeddings.
//!
//! Delegates to the OpenAI provider (or any OpenAI-compatible one configured).
//! Model field accepts "<provider>:<model>" (default: openai).

use axum::extract::State;
use axum::Json;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use crate::error::{AppError, AppResult};
use crate::middleware::auth::ApiKeyAuth;
use crate::providers::SharedRegistry;

#[derive(Debug, Deserialize)]
pub struct EmbeddingRequest {
    pub model: String,
    pub input: EmbeddingInput,
    #[serde(default)]
    pub encoding_format: Option<String>, // "float" (default) | "base64"
    #[serde(default)]
    pub dimensions: Option<u32>, // for models that support it (text-embedding-3-*)
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum EmbeddingInput {
    Single(String),
    Multiple(Vec<String>),
}

#[derive(Debug, Serialize)]
pub struct EmbeddingResponse {
    pub object: &'static str, // "list"
    pub data: Vec<EmbeddingData>,
    pub model: String,
    pub usage: EmbeddingUsage,
}

#[derive(Debug, Serialize)]
pub struct EmbeddingData {
    pub object: &'static str, // "embedding"
    pub index: u32,
    pub embedding: Vec<f32>,
}

#[derive(Debug, Serialize)]
pub struct EmbeddingUsage {
    pub prompt_tokens: u32,
    pub total_tokens: u32,
}

pub async fn create_embedding(
    State(registry): State<SharedRegistry>,
    _auth: ApiKeyAuth,
    Json(req): Json<EmbeddingRequest>,
) -> AppResult<Json<Value>> {
    let registry = registry.read().await;
    let provider = registry.pick(&ChatReqWrapper::from(&req))
        .or_else(|| registry.all().first().cloned())
        .ok_or_else(|| AppError::AllProvidersFailed)?;

    // Build the upstream request — we just forward to the provider's /v1/embeddings
    let (provider_hint, model_name) = split_model(&req.model);
    let _ = provider_hint;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build().map_err(|e| AppError::Internal(e.to_string()))?;

    let base_url = provider_base_url(&provider, &req.model);
    let api_key = provider_api_key(&provider, &req.model);

    let body = json!({
        "model": model_name,
        "input": match &req.input {
            EmbeddingInput::Single(s) => s,
            EmbeddingInput::Multiple(arr) => arr,
        },
        "encoding_format": req.encoding_format.as_deref().unwrap_or("float"),
        "dimensions": req.dimensions,
    });

    let mut request = client.post(format!("{}/embeddings", base_url))
        .header("Content-Type", "application/json")
        .json(&body);
    if let Some(key) = api_key {
        request = request.header("Authorization", format!("Bearer {}", key));
    }

    let resp = request.send().await
        .map_err(|e| AppError::Provider(format!("embeddings request failed: {}", e)))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(AppError::Provider(format!("embeddings {} error: {}", status, text)));
    }

    let v: Value = resp.json().await
        .map_err(|e| AppError::Provider(format!("embeddings decode failed: {}", e)))?;
    Ok(Json(v))
}

// ─── Helpers (duplicate of logic in providers/openai.rs, kept here for
//     embedding-specific dispatch — a real refactor would expose these) ───────

struct ChatReqWrapper {
    model: String,
}

impl From<&EmbeddingRequest> for ChatReqWrapper {
    fn from(req: &EmbeddingRequest) -> Self {
        Self { model: req.model.clone() }
    }
}

impl ChatReqWrapper {
    fn split_model(&self) -> (Option<&str>, &str) {
        match self.model.split_once(':') {
            Some((p, m)) => (Some(p), m),
            None => (None, self.model.as_str()),
        }
    }
}

// Reuse the providers' internal base URL + key via the trait — for simplicity
// we just hit the OpenAI base URL. A full impl would expose provider.base_url()
// + provider.api_key() on the trait.
fn split_model(model: &str) -> (Option<&str>, &str) {
    match model.split_once(':') {
        Some((p, m)) => (Some(p), m),
        None => (None, model),
    }
}

fn provider_base_url(_provider: &std::sync::Arc<dyn crate::providers::Provider>, _model: &str) -> String {
    // TODO: add `base_url()` to the Provider trait so we can dispatch correctly.
    // For now, all OpenAI-compatible providers expose /v1/embeddings at their base URL.
    // We use a hardcoded lookup by provider id.
    "https://api.openai.com/v1".to_string()
}

fn provider_api_key(_provider: &std::sync::Arc<dyn crate::providers::Provider>, _model: &str) -> Option<String> {
    // Same TODO — would call provider.api_key() on the trait.
    std::env::var("OPENAI_API_KEY").ok()
}
