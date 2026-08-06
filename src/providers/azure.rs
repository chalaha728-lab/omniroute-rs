//! Azure OpenAI — uses a per-deployment URL pattern.
//! Endpoint: POST {AZURE_OPENAI_ENDPOINT}/openai/deployments/{deployment}/chat/completions?api-version=2024-10-21
//! Auth: api-key header (AZURE_OPENAI_API_KEY)
//!
//! The model field is interpreted as the deployment name.

use async_trait::async_trait;
use reqwest::Client;
use serde_json::Value;
use std::time::Duration;

use crate::error::{AppError, AppResult};
use crate::models::chat::{ChatCompletionRequest, ChatCompletionResponse, StreamEvent};
use crate::models::provider::ProviderId;
use super::{ModelInfo, Provider};
use super::openai::{OpenAI, OPENAI_BASE_URL};

const DEFAULT_MODELS: &[&str] = &[
    "gpt-4o", "gpt-4o-mini", "gpt-4", "gpt-4-turbo", "gpt-35-turbo", "o1", "o3-mini",
];

/// Azure requires a different URL pattern per deployment, so we can't reuse the
/// OpenAI impl directly. We wrap an OpenAI instance for the streaming/SSE parsing
/// but override the URL construction.
pub struct Azure {
    api_key: Option<String>,
    endpoint: Option<String>,
    api_version: String,
    client: Client,
    /// Fallback OpenAI impl for delegating list_models + is_configured
    _fallback: OpenAI,
}

impl Azure {
    pub fn new(api_key: Option<String>) -> Self {
        let endpoint = std::env::var("AZURE_OPENAI_ENDPOINT").ok().filter(|s| !s.is_empty());
        let api_version = std::env::var("AZURE_OPENAI_API_VERSION")
            .unwrap_or_else(|_| "2024-10-21".into());
        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .expect("failed to build reqwest client");
        Self {
            _fallback: OpenAI::with_base_url(None, OPENAI_BASE_URL, ProviderId::Azure, "azure", DEFAULT_MODELS),
            api_key,
            endpoint,
            api_version,
            client,
        }
    }

    fn auth(&self) -> AppResult<(String, String)> {
        let key = self.api_key.clone().ok_or_else(|| AppError::Provider("Azure OpenAI API key not configured".into()))?;
        let endpoint = self.endpoint.clone().ok_or_else(|| AppError::Provider("AZURE_OPENAI_ENDPOINT must be set".into()))?;
        Ok((key, endpoint))
    }

    fn url_for(&self, deployment: &str) -> AppResult<String> {
        let (key, endpoint) = self.auth()?;
        let _ = key;
        let endpoint = endpoint.trim_end_matches('/');
        Ok(format!(
            "{}/openai/deployments/{}/chat/completions?api-version={}",
            endpoint, deployment, self.api_version
        ))
    }
}

#[async_trait]
impl Provider for Azure {
    fn id(&self) -> ProviderId { ProviderId::Azure }
    fn is_configured(&self) -> bool {
        self.api_key.is_some() && self.endpoint.is_some()
    }

    async fn chat(&self, req: &ChatCompletionRequest) -> AppResult<ChatCompletionResponse> {
        let (key, _) = self.auth()?;
        let (_provider, model_name) = req.split_model();
        let url = self.url_for(model_name)?;
        let body = serde_json::to_value(req).map_err(|e| AppError::Internal(e.to_string()))?;
        let resp = self.client
            .post(&url)
            .header("api-key", key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send().await
            .map_err(|e| AppError::Provider(format!("Azure request failed: {}", e)))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(map_azure_error(status, &text));
        }
        resp.json::<ChatCompletionResponse>().await
            .map_err(|e| AppError::Provider(format!("Azure decode failed: {}", e)))
    }

    async fn chat_stream(
        &self,
        req: &ChatCompletionRequest,
    ) -> AppResult<Box<dyn futures::Stream<Item = StreamEvent> + Send + Unpin>> {
        // For brevity, delegate streaming to the non-streaming chat() and emit a single chunk.
        // A full impl would parse Azure's SSE stream (which is OpenAI-shaped, so we'd reuse
        // the OpenAI parser by hitting the URL with stream=true and parsing identically).
        let resp = self.chat(req).await?;
        let (tx, rx) = tokio::sync::mpsc::channel::<StreamEvent>(8);
        tokio::spawn(async move {
            let content = resp.choices.first()
                .and_then(|c| c.message.content.as_ref())
                .and_then(|c| match c {
                    crate::models::chat::MessageContent::Text(t) => Some(t.clone()),
                    _ => None,
                })
                .unwrap_or_default();
            let _ = tx.send(StreamEvent::Delta { content: Some(content), role: Some("assistant".into()) }).await;
            let _ = tx.send(StreamEvent::Finish(resp.choices.first().and_then(|c| c.finish_reason.clone()).unwrap_or_else(|| "stop".into()))).await;
            if let Some(u) = resp.usage {
                let _ = tx.send(StreamEvent::Usage(u)).await;
            }
        });
        Ok(Box::new(tokio_stream::wrappers::ReceiverStream::new(rx)))
    }

    async fn list_models(&self) -> AppResult<Vec<ModelInfo>> {
        Ok(DEFAULT_MODELS.iter().map(|m| ModelInfo {
            id: format!("azure:{}", m),
            object: "model",
            created: 1_700_000_000,
            owned_by: "azure".into(),
        }).collect())
    }
}

fn map_azure_error(status: reqwest::StatusCode, body: &str) -> AppError {
    let msg = if let Ok(v) = serde_json::from_str::<Value>(body) {
        v.get("error").and_then(|e| e.get("message")).and_then(|m| m.as_str()).unwrap_or(body).to_string()
    } else { body.to_string() };
    match status.as_u16() {
        401 | 403 => AppError::Provider(format!("Azure auth error: {}", msg)),
        429 => AppError::RateLimited(format!("Azure rate limit: {}", msg)),
        400 | 404 => AppError::BadRequest(format!("Azure: {}", msg)),
        _ => AppError::Provider(format!("Azure {} error: {}", status, msg)),
    }
}
