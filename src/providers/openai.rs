//! OpenAI provider — the canonical OpenAI-compatible implementation.
//!
//! Endpoints:
//!   POST https://api.openai.com/v1/chat/completions
//!   GET  https://api.openai.com/v1/models
//!
//! Other OpenAI-compatible providers (DeepSeek, OpenRouter) reuse this code
//! via `OpenAI::with_base_url(...)`.

use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use serde_json::Value;
use std::time::Duration;
use tokio_stream::wrappers::ReceiverStream;

use crate::error::{AppError, AppResult};
use crate::models::chat::{ChatCompletionRequest, ChatCompletionResponse, StreamEvent, Usage};
use crate::models::provider::ProviderId;
use super::{ModelInfo, Provider};

pub const OPENAI_BASE_URL: &str = "https://api.openai.com/v1";
const DEFAULT_MODELS: &[&str] = &[
    "gpt-4o", "gpt-4o-mini", "gpt-4-turbo", "gpt-4", "gpt-3.5-turbo",
    "o1", "o1-mini", "o1-preview", "o3-mini",
];

pub struct OpenAI {
    api_key: Option<String>,
    client: Client,
    base_url: String,
    provider_id: ProviderId,
    owned_by: String,
    default_models: &'static [&'static str],
}

impl OpenAI {
    /// Create with the default OpenAI base URL.
    pub fn new(api_key: Option<String>) -> Self {
        Self::with_base_url(
            api_key,
            OPENAI_BASE_URL,
            ProviderId::OpenAI,
            "openai",
            DEFAULT_MODELS,
        )
    }

    /// Create with a custom base URL (used by DeepSeek, OpenRouter, etc.).
    pub fn with_base_url(
        api_key: Option<String>,
        base_url: impl Into<String>,
        provider_id: ProviderId,
        owned_by: &'static str,
        default_models: &'static [&'static str],
    ) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .expect("failed to build reqwest client");
        Self {
            api_key,
            client,
            base_url: base_url.into(),
            provider_id,
            owned_by: owned_by.into(),
            default_models,
        }
    }

    fn auth_header(&self) -> AppResult<String> {
        let key = self.api_key.as_ref().ok_or_else(|| {
            AppError::Provider(format!("{} API key not configured", self.provider_id))
        })?;
        Ok(format!("Bearer {}", key))
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }
}

#[async_trait]
impl Provider for OpenAI {
    fn id(&self) -> ProviderId { self.provider_id }
    fn is_configured(&self) -> bool { self.api_key.is_some() }

    async fn chat(&self, req: &ChatCompletionRequest) -> AppResult<ChatCompletionResponse> {
        let body = serde_json::to_value(req).map_err(|e| AppError::Internal(e.to_string()))?;
        let auth = self.auth_header()?;

        let resp = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", auth)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::Provider(format!("{} request failed: {}", self.provider_id, e)))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(map_openai_error(self.provider_id, status, &text));
        }

        resp.json::<ChatCompletionResponse>()
            .await
            .map_err(|e| AppError::Provider(format!("{} decode failed: {}", self.provider_id, e)))
    }

    async fn chat_stream(
        &self,
        req: &ChatCompletionRequest,
    ) -> AppResult<Box<dyn futures::Stream<Item = StreamEvent> + Send + Unpin>> {
        let mut body = serde_json::to_value(req).map_err(|e| AppError::Internal(e.to_string()))?;
        body["stream"] = Value::Bool(true);
        body["stream_options"] = serde_json::json!({ "include_usage": true });

        let auth = self.auth_header()?;
        let resp = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", auth)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::Provider(format!("{} stream request failed: {}", self.provider_id, e)))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(map_openai_error(self.provider_id, status, &text));
        }

        let (tx, rx) = tokio::sync::mpsc::channel::<StreamEvent>(64);
        let mut byte_stream = resp.bytes_stream();
        let pid = self.provider_id;

        tokio::spawn(async move {
            let mut buf = String::new();
            while let Some(chunk_res) = byte_stream.next().await {
                match chunk_res {
                    Ok(bytes) => {
                        buf.push_str(&String::from_utf8_lossy(&bytes));
                        while let Some(idx) = buf.find("\n\n") {
                            let event_str = buf[..idx].to_string();
                            buf.drain(..idx + 2);
                            if let Some(event) = parse_sse_event(&event_str) {
                                if tx.send(event).await.is_err() {
                                    return;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(StreamEvent::Error(format!("stream error: {}", e))).await;
                        return;
                    }
                }
            }
            let _ = pid; // suppress unused warning
        });

        Ok(Box::new(ReceiverStream::new(rx)))
    }

    async fn list_models(&self) -> AppResult<Vec<ModelInfo>> {
        Ok(self.default_models.iter().map(|m| ModelInfo {
            id: format!("{}:{}", self.provider_id, m),
            object: "model",
            created: 1_700_000_000,
            owned_by: self.owned_by.clone(),
        }).collect())
    }
}

fn parse_sse_event(raw: &str) -> Option<StreamEvent> {
    for line in raw.lines() {
        let line = line.trim();
        if let Some(data) = line.strip_prefix("data: ") {
            let data = data.trim();
            if data == "[DONE]" {
                return Some(StreamEvent::Finish("stop".into()));
            }
            if let Ok(v) = serde_json::from_str::<Value>(data) {
                if let Some(err) = v.get("error") {
                    let msg = err.get("message").and_then(|m| m.as_str()).unwrap_or("unknown error");
                    return Some(StreamEvent::Error(msg.into()));
                }
                if let Some(usage) = v.get("usage") {
                    if !usage.is_null() {
                        let prompt = usage.get("prompt_tokens").and_then(|t| t.as_u64()).unwrap_or(0) as u32;
                        let completion = usage.get("completion_tokens").and_then(|t| t.as_u64()).unwrap_or(0) as u32;
                        return Some(StreamEvent::Usage(Usage {
                            prompt_tokens: prompt,
                            completion_tokens: completion,
                            total_tokens: prompt + completion,
                        }));
                    }
                }
                if let Some(choices) = v.get("choices").and_then(|c| c.as_array()) {
                    if let Some(choice) = choices.first() {
                        let delta = choice.get("delta");
                        let finish = choice.get("finish_reason").and_then(|f| f.as_str());
                        if let Some(finish) = finish {
                            return Some(StreamEvent::Finish(finish.into()));
                        }
                        if let Some(delta) = delta {
                            let content = delta.get("content").and_then(|c| c.as_str()).map(|s| s.to_string());
                            let role = delta.get("role").and_then(|r| r.as_str()).map(|s| s.to_string());
                            if content.is_some() || role.is_some() {
                                return Some(StreamEvent::Delta { content, role });
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

fn map_openai_error(provider: ProviderId, status: reqwest::StatusCode, body: &str) -> AppError {
    let msg = if let Ok(v) = serde_json::from_str::<Value>(body) {
        v.get("error")
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
            .unwrap_or(body)
            .to_string()
    } else {
        body.to_string()
    };
    match status.as_u16() {
        401 | 403 => AppError::Provider(format!("{} auth error: {}", provider, msg)),
        429 => AppError::RateLimited(format!("{} rate limit: {}", provider, msg)),
        400 | 404 => AppError::BadRequest(format!("{}: {}", provider, msg)),
        _ => AppError::Provider(format!("{} {} error: {}", provider, status, msg)),
    }
}
