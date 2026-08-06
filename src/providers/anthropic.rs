//! Anthropic (Claude) provider.
//!
//! Endpoints:
//!   POST https://api.anthropic.com/v1/messages
//!   Anthropic uses x-api-key header (not Bearer) and requires anthropic-version.
//!
//! Message format conversion:
//!   OpenAI:  messages=[{role:"system",content:"..."},{role:"user",content:"..."}]
//!   Anthropic: system="..." + messages=[{role:"user",content:"..."}]

use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use serde_json::{json, Value};
use std::time::Duration;
use tokio_stream::wrappers::ReceiverStream;

use crate::error::{AppError, AppResult};
use crate::models::chat::{
    ChatCompletionRequest, ChatCompletionResponse, Message, StreamEvent, Usage,
};
use crate::models::provider::ProviderId;
use super::{ModelInfo, Provider};

const BASE_URL: &str = "https://api.anthropic.com/v1";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const DEFAULT_MODELS: &[&str] = &[
    "claude-3-5-sonnet-20241022",
    "claude-3-5-haiku-20241022",
    "claude-3-opus-20240229",
    "claude-3-sonnet-20240229",
    "claude-3-haiku-20240307",
];

pub struct Anthropic {
    api_key: Option<String>,
    client: Client,
}

impl Anthropic {
    pub fn new(api_key: Option<String>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .expect("failed to build reqwest client");
        Self { api_key, client }
    }

    fn auth(&self) -> AppResult<(String, &'static str)> {
        let key = self.api_key.as_ref().ok_or_else(|| {
            AppError::Provider("Anthropic API key not configured".into())
        })?;
        Ok((key.clone(), ANTHROPIC_VERSION))
    }

    /// Convert OpenAI messages to Anthropic format (system extracted to top-level).
    fn convert_messages(messages: &[Message]) -> (Option<String>, Vec<Value>) {
        let mut system: Option<String> = None;
        let mut out: Vec<Value> = Vec::with_capacity(messages.len());

        for msg in messages {
            match msg.role.as_str() {
                "system" => {
                    let text = msg.content.as_ref().map(|c| match c {
                        crate::models::chat::MessageContent::Text(t) => t.clone(),
                        crate::models::chat::MessageContent::Parts(parts) => parts
                            .iter()
                            .filter_map(|p| match p {
                                crate::models::chat::ContentPart::Text { text } => Some(text.clone()),
                                _ => None,
                            })
                            .collect::<Vec<_>>()
                            .join(""),
                    }).unwrap_or_default();
                    system = Some(if let Some(s) = system {
                        format!("{}\n\n{}", s, text)
                    } else {
                        text
                    });
                }
                "user" | "assistant" => {
                    let content = msg.content.as_ref().map(|c| match c {
                        crate::models::chat::MessageContent::Text(t) => {
                            json!([{ "type": "text", "text": t }])
                        }
                        crate::models::chat::MessageContent::Parts(parts) => {
                            let arr: Vec<Value> = parts.iter().map(|p| match p {
                                crate::models::chat::ContentPart::Text { text } => json!({
                                    "type": "text", "text": text
                                }),
                                crate::models::chat::ContentPart::ImageUrl { image_url } => json!({
                                    "type": "image",
                                    "source": {
                                        "type": "base64",
                                        "media_type": "image/jpeg",
                                        "data": image_url.url.strip_prefix("data:image/jpeg;base64,").unwrap_or(&image_url.url)
                                    }
                                }),
                            }).collect();
                            Value::Array(arr)
                        }
                    }).unwrap_or(Value::Null);

                    out.push(json!({
                        "role": msg.role,
                        "content": content,
                    }));
                }
                "tool" => {
                    // Tool result — Anthropic format is {role:"user", content:[{type:"tool_result",tool_use_id,content}]}
                    let tool_call_id = msg.tool_call_id.clone().unwrap_or_default();
                    let text = msg.content.as_ref().map(|c| match c {
                        crate::models::chat::MessageContent::Text(t) => t.clone(),
                        _ => String::new(),
                    }).unwrap_or_default();
                    out.push(json!({
                        "role": "user",
                        "content": [{
                            "type": "tool_result",
                            "tool_use_id": tool_call_id,
                            "content": text,
                        }]
                    }));
                }
                _ => {}
            }
        }
        (system, out)
    }

    /// Convert Anthropic response to OpenAI ChatCompletionResponse.
    fn convert_response(resp: &Value, model: &str) -> ChatCompletionResponse {
        let id = resp.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let content = resp
            .get("content")
            .and_then(|c| c.as_array())
            .map(|arr| {
                let text: String = arr.iter()
                    .filter_map(|block| {
                        if block.get("type").and_then(|t| t.as_str()) == Some("text") {
                            block.get("text").and_then(|t| t.as_str()).map(|s| s.to_string())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("");
                text
            })
            .unwrap_or_default();

        let stop_reason = resp.get("stop_reason").and_then(|s| s.as_str()).unwrap_or("stop");
        let finish = match stop_reason {
            "end_turn" => "stop",
            "max_tokens" => "length",
            "tool_use" => "tool_calls",
            other => other,
        };

        let usage = resp.get("usage").map(|u| {
            let prompt = u.get("input_tokens").and_then(|t| t.as_u64()).unwrap_or(0) as u32;
            let completion = u.get("output_tokens").and_then(|t| t.as_u64()).unwrap_or(0) as u32;
            Usage {
                prompt_tokens: prompt,
                completion_tokens: completion,
                total_tokens: prompt + completion,
            }
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
                    content: Some(crate::models::chat::MessageContent::Text(content)),
                    tool_calls: None,
                    tool_call_id: None,
                    name: None,
                },
                finish_reason: Some(finish.into()),
            }],
            usage,
            system_fingerprint: None,
        }
    }
}

#[async_trait]
impl Provider for Anthropic {
    fn id(&self) -> ProviderId { ProviderId::Anthropic }
    fn is_configured(&self) -> bool { self.api_key.is_some() }

    async fn chat(&self, req: &ChatCompletionRequest) -> AppResult<ChatCompletionResponse> {
        let (key, version) = self.auth()?;
        let (system, messages) = Self::convert_messages(&req.messages);
        let (_provider_hint, model_name) = req.split_model();
        let model = model_name.to_string();

        let mut body = json!({
            "model": model,
            "messages": messages,
            "max_tokens": req.max_tokens.unwrap_or(4096),
            "temperature": req.temperature,
            "top_p": req.top_p,
        });
        if let Some(s) = system {
            body["system"] = Value::String(s);
        }
        if let Some(stop) = &req.stop {
            match stop {
                crate::models::chat::StopSequence::Single(s) => body["stop_sequences"] = json!([s]),
                crate::models::chat::StopSequence::Multiple(arr) => body["stop_sequences"] = json!(arr),
            }
        }

        let resp = self
            .client
            .post(format!("{}/messages", BASE_URL))
            .header("x-api-key", key)
            .header("anthropic-version", version)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::Provider(format!("Anthropic request failed: {}", e)))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(map_anthropic_error(status, &text));
        }

        let v: Value = resp.json().await
            .map_err(|e| AppError::Provider(format!("Anthropic decode failed: {}", e)))?;
        Ok(Self::convert_response(&v, &model))
    }

    async fn chat_stream(
        &self,
        req: &ChatCompletionRequest,
    ) -> AppResult<Box<dyn futures::Stream<Item = StreamEvent> + Send + Unpin>> {
        let (key, version) = self.auth()?;
        let (system, messages) = Self::convert_messages(&req.messages);
        let (_provider_hint, model_name) = req.split_model();
        let model = model_name.to_string();

        let mut body = json!({
            "model": model,
            "messages": messages,
            "max_tokens": req.max_tokens.unwrap_or(4096),
            "temperature": req.temperature,
            "top_p": req.top_p,
            "stream": true,
        });
        if let Some(s) = system {
            body["system"] = Value::String(s);
        }

        let resp = self
            .client
            .post(format!("{}/messages", BASE_URL))
            .header("x-api-key", key)
            .header("anthropic-version", version)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::Provider(format!("Anthropic stream request failed: {}", e)))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(map_anthropic_error(status, &text));
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
                            if let Some(event) = parse_anthropic_event(&event_str) {
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
        });

        Ok(Box::new(ReceiverStream::new(rx)))
    }

    async fn list_models(&self) -> AppResult<Vec<ModelInfo>> {
        Ok(DEFAULT_MODELS.iter().map(|m| ModelInfo {
            id: format!("anthropic:{}", m),
            object: "model",
            created: 1_700_000_000,
            owned_by: "anthropic".into(),
        }).collect())
    }
}

/// Parse an Anthropic SSE event into a StreamEvent.
/// Anthropic event types: message_start, content_block_start, content_block_delta,
/// content_block_stop, message_delta, message_stop.
fn parse_anthropic_event(raw: &str) -> Option<StreamEvent> {
    let mut event_type = String::new();
    let mut data = String::new();
    for line in raw.lines() {
        let line = line.trim();
        if let Some(t) = line.strip_prefix("event: ") {
            event_type = t.trim().to_string();
        } else if let Some(d) = line.strip_prefix("data: ") {
            data.push_str(d.trim());
        }
    }
    if data.is_empty() {
        return None;
    }
    let v: Value = match serde_json::from_str(&data) {
        Ok(v) => v,
        Err(_) => return None,
    };

    match event_type.as_str() {
        "content_block_delta" => {
            let delta = v.get("delta")?;
            if delta.get("type").and_then(|t| t.as_str()) == Some("text_delta") {
                let text = delta.get("text").and_then(|t| t.as_str()).map(|s| s.to_string());
                return Some(StreamEvent::Delta { content: text, role: None });
            }
            None
        }
        "message_start" => {
            // Initial role marker
            Some(StreamEvent::Delta { content: None, role: Some("assistant".into()) })
        }
        "message_delta" => {
            let stop_reason = v.get("delta")
                .and_then(|d| d.get("stop_reason"))
                .and_then(|s| s.as_str());
            if let Some(reason) = stop_reason {
                let finish = match reason {
                    "end_turn" => "stop",
                    "max_tokens" => "length",
                    "tool_use" => "tool_calls",
                    other => other,
                };
                return Some(StreamEvent::Finish(finish.into()));
            }
            // Maybe usage
            if let Some(usage) = v.get("usage") {
                let prompt = usage.get("input_tokens").and_then(|t| t.as_u64()).unwrap_or(0) as u32;
                let completion = usage.get("output_tokens").and_then(|t| t.as_u64()).unwrap_or(0) as u32;
                return Some(StreamEvent::Usage(Usage {
                    prompt_tokens: prompt,
                    completion_tokens: completion,
                    total_tokens: prompt + completion,
                }));
            }
            None
        }
        "message_stop" => Some(StreamEvent::Finish("stop".into())),
        "error" => {
            let msg = v.get("error")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .unwrap_or("anthropic stream error");
            Some(StreamEvent::Error(msg.into()))
        }
        _ => None,
    }
}

fn map_anthropic_error(status: reqwest::StatusCode, body: &str) -> AppError {
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
        401 | 403 => AppError::Provider(format!("Anthropic auth error: {}", msg)),
        429 => AppError::RateLimited(format!("Anthropic rate limit: {}", msg)),
        400 | 404 => AppError::BadRequest(format!("Anthropic: {}", msg)),
        _ => AppError::Provider(format!("Anthropic {} error: {}", status, msg)),
    }
}
