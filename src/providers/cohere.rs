//! Cohere — native format (v2 chat API).
//! Endpoint: POST https://api.cohere.com/v2/chat
//! Auth: Bearer token

use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use serde_json::{json, Value};
use std::time::Duration;
use tokio_stream::wrappers::ReceiverStream;

use crate::error::{AppError, AppResult};
use crate::models::chat::{
    ChatCompletionRequest, ChatCompletionResponse, Message, MessageContent, StreamEvent, Usage,
};
use crate::models::provider::ProviderId;
use super::{ModelInfo, Provider};

const BASE_URL: &str = "https://api.cohere.com/v2";
const DEFAULT_MODELS: &[&str] = &[
    "command-r-plus-08-2024",
    "command-r-08-2024",
    "command-r7b-12-2024",
    "c4ai-aya-expanse-8b",
    "c4ai-aya-expanse-32b",
];

pub struct Cohere {
    api_key: Option<String>,
    client: Client,
}

impl Cohere {
    pub fn new(api_key: Option<String>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .expect("failed to build reqwest client");
        Self { api_key, client }
    }

    fn auth(&self) -> AppResult<String> {
        self.api_key.clone().ok_or_else(|| {
            AppError::Provider("Cohere API key not configured".into())
        })
    }

    fn convert_messages(messages: &[Message]) -> Vec<Value> {
        messages.iter().filter_map(|msg| {
            let role = match msg.role.as_str() {
                "system" => "system".to_string(),
                "user" => "user".to_string(),
                "assistant" => "assistant".to_string(),
                "tool" => "tool".to_string(),
                _ => return None,
            };
            let content = match &msg.content {
                Some(MessageContent::Text(t)) => t.clone(),
                Some(MessageContent::Parts(parts)) => parts.iter()
                    .filter_map(|p| match p {
                        crate::models::chat::ContentPart::Text { text } => Some(text.clone()),
                        _ => None,
                    }).collect::<Vec<_>>().join(""),
                None => String::new(),
            };
            Some(json!({ "role": role, "content": content }))
        }).collect()
    }

    fn convert_response(v: &Value, model: &str) -> ChatCompletionResponse {
        let id = v.get("id").and_then(|i| i.as_str()).unwrap_or("").to_string();
        let content = v.get("message")
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_array())
            .map(|arr| arr.iter()
                .filter_map(|b| b.get("text").and_then(|t| t.as_str()).map(|s| s.to_string()))
                .collect::<Vec<_>>()
                .join(""))
            .unwrap_or_default();
        let finish = v.get("finish_reason").and_then(|f| f.as_str()).unwrap_or("stop");
        let usage = v.get("usage").map(|u| {
            let prompt = u.get("billed_units").and_then(|b| b.get("input_tokens")).and_then(|t| t.as_u64()).unwrap_or(0) as u32;
            let completion = u.get("billed_units").and_then(|b| b.get("output_tokens")).and_then(|t| t.as_u64()).unwrap_or(0) as u32;
            Usage { prompt_tokens: prompt, completion_tokens: completion, total_tokens: prompt + completion }
        });
        ChatCompletionResponse {
            id,
            object: "chat.completion",
            created: chrono::Utc::now().timestamp(),
            model: model.into(),
            choices: vec![crate::models::chat::Choice {
                index: 0,
                message: Message {
                    role: "assistant".into(),
                    content: Some(MessageContent::Text(content)),
                    tool_calls: None, tool_call_id: None, name: None,
                },
                finish_reason: Some(finish.into()),
            }],
            usage,
            system_fingerprint: None,
        }
    }
}

#[async_trait]
impl Provider for Cohere {
    fn id(&self) -> ProviderId { ProviderId::Cohere }
    fn is_configured(&self) -> bool { self.api_key.is_some() }

    async fn chat(&self, req: &ChatCompletionRequest) -> AppResult<ChatCompletionResponse> {
        let key = self.auth()?;
        let (_provider, model_name) = req.split_model();
        let model = model_name.to_string();
        let body = json!({
            "model": model,
            "messages": Self::convert_messages(&req.messages),
            "temperature": req.temperature,
            "max_tokens": req.max_tokens.unwrap_or(4096),
        });
        let resp = self.client
            .post(format!("{}/chat", BASE_URL))
            .header("Authorization", format!("Bearer {}", key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send().await
            .map_err(|e| AppError::Provider(format!("Cohere request failed: {}", e)))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(map_cohere_error(status, &text));
        }
        let v: Value = resp.json().await
            .map_err(|e| AppError::Provider(format!("Cohere decode failed: {}", e)))?;
        Ok(Self::convert_response(&v, &model))
    }

    async fn chat_stream(
        &self,
        req: &ChatCompletionRequest,
    ) -> AppResult<Box<dyn futures::Stream<Item = StreamEvent> + Send + Unpin>> {
        let key = self.auth()?;
        let (_provider, model_name) = req.split_model();
        let model = model_name.to_string();
        let body = json!({
            "model": model,
            "messages": Self::convert_messages(&req.messages),
            "temperature": req.temperature,
            "max_tokens": req.max_tokens.unwrap_or(4096),
            "stream": true,
        });
        let resp = self.client
            .post(format!("{}/chat", BASE_URL))
            .header("Authorization", format!("Bearer {}", key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send().await
            .map_err(|e| AppError::Provider(format!("Cohere stream request failed: {}", e)))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(map_cohere_error(status, &text));
        }
        let (tx, rx) = tokio::sync::mpsc::channel::<StreamEvent>(64);
        let mut byte_stream = resp.bytes_stream();
        tokio::spawn(async move {
            let mut buf = String::new();
            while let Some(chunk_res) = byte_stream.next().await {
                match chunk_res {
                    Ok(bytes) => {
                        buf.push_str(&String::from_utf8_lossy(&bytes));
                        while let Some(idx) = buf.find("\n\n") {
                            let event_str = buf[..idx].to_string();
                            buf.drain(..idx + 2);
                            if let Some(event) = parse_cohere_event(&event_str) {
                                if tx.send(event).await.is_err() { return; }
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(StreamEvent::Error(format!("stream error: {}", e))).await;
                        return;
                    }
                }
            }
            let _ = tx.send(StreamEvent::Finish("stop".into())).await;
        });
        Ok(Box::new(ReceiverStream::new(rx)))
    }

    async fn list_models(&self) -> AppResult<Vec<ModelInfo>> {
        Ok(DEFAULT_MODELS.iter().map(|m| ModelInfo {
            id: format!("cohere:{}", m),
            object: "model",
            created: 1_700_000_000,
            owned_by: "cohere".into(),
        }).collect())
    }
}

fn parse_cohere_event(raw: &str) -> Option<StreamEvent> {
    for line in raw.lines() {
        let line = line.trim();
        if let Some(data) = line.strip_prefix("data: ") {
            let data = data.trim();
            if let Ok(v) = serde_json::from_str::<Value>(data) {
                if let Some(delta) = v.get("delta") {
                    if let Some(msg) = delta.get("message") {
                        if let Some(content) = msg.get("content").and_then(|c| c.as_array()) {
                            let text = content.iter()
                                .filter_map(|b| b.get("text").and_then(|t| t.as_str()).map(|s| s.to_string()))
                                .collect::<Vec<_>>()
                                .join("");
                            if !text.is_empty() {
                                return Some(StreamEvent::Delta { content: Some(text), role: None });
                            }
                        }
                    }
                }
                if let Some(event_type) = v.get("type").and_then(|t| t.as_str()) {
                    if event_type == "message-end" {
                        if let Some(usage) = v.get("usage") {
                            let prompt = usage.get("billed_units").and_then(|b| b.get("input_tokens")).and_then(|t| t.as_u64()).unwrap_or(0) as u32;
                            let completion = usage.get("billed_units").and_then(|b| b.get("output_tokens")).and_then(|t| t.as_u64()).unwrap_or(0) as u32;
                            return Some(StreamEvent::Usage(Usage {
                                prompt_tokens: prompt,
                                completion_tokens: completion,
                                total_tokens: prompt + completion,
                            }));
                        }
                        return Some(StreamEvent::Finish("stop".into()));
                    }
                }
            }
        }
    }
    None
}

fn map_cohere_error(status: reqwest::StatusCode, body: &str) -> AppError {
    let msg = if let Ok(v) = serde_json::from_str::<Value>(body) {
        v.get("message").and_then(|m| m.as_str()).unwrap_or(body).to_string()
    } else { body.to_string() };
    match status.as_u16() {
        401 | 403 => AppError::Provider(format!("Cohere auth error: {}", msg)),
        429 => AppError::RateLimited(format!("Cohere rate limit: {}", msg)),
        400 | 404 => AppError::BadRequest(format!("Cohere: {}", msg)),
        _ => AppError::Provider(format!("Cohere {} error: {}", status, msg)),
    }
}
