//! Google Gemini provider.
//!
//! Endpoints:
//!   POST https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent
//!   POST https://generativelanguage.googleapis.com/v1beta/models/{model}:streamGenerateContent?alt=sse
//!
//! Message format conversion:
//!   OpenAI:  messages=[{role:"user",content:"..."}]
//!   Gemini:  contents=[{role:"user",parts:[{text:"..."}]}]
//!   System instruction: systemInstruction={parts:[{text:"..."}]}

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

const BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";
const DEFAULT_MODELS: &[&str] = &[
    "gemini-2.0-flash-exp",
    "gemini-1.5-pro",
    "gemini-1.5-flash",
    "gemini-1.5-flash-8b",
];

pub struct Gemini {
    api_key: Option<String>,
    client: Client,
}

impl Gemini {
    pub fn new(api_key: Option<String>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .expect("failed to build reqwest client");
        Self { api_key, client }
    }

    fn key(&self) -> AppResult<String> {
        self.api_key.clone().ok_or_else(|| {
            AppError::Provider("Gemini API key not configured".into())
        })
    }

    /// Convert OpenAI messages → Gemini contents + systemInstruction.
    fn convert_messages(messages: &[Message]) -> (Option<Value>, Vec<Value>) {
        let mut system: Option<Value> = None;
        let mut contents: Vec<Value> = Vec::with_capacity(messages.len());

        for msg in messages {
            match msg.role.as_str() {
                "system" => {
                    let text = extract_text(&msg.content);
                    system = Some(json!({ "parts": [{ "text": text }] }));
                }
                "user" | "assistant" => {
                    let role = if msg.role == "assistant" { "model" } else { "user" };
                    let parts = match &msg.content {
                        Some(MessageContent::Text(t)) => vec![json!({ "text": t })],
                        Some(MessageContent::Parts(arr)) => arr.iter().map(|p| match p {
                            crate::models::chat::ContentPart::Text { text } => json!({ "text": text }),
                            crate::models::chat::ContentPart::ImageUrl { image_url } => {
                                // Parse data URL: data:image/jpeg;base64,....
                                if let Some((mime, b64)) = image_url.url.split_once(";base64,") {
                                    let mime = mime.strip_prefix("data:").unwrap_or(mime);
                                    json!({
                                        "inline_data": {
                                            "mime_type": mime,
                                            "data": b64,
                                        }
                                    })
                                } else {
                                    json!({ "text": image_url.url })
                                }
                            }
                        }).collect(),
                        None => vec![],
                    };
                    contents.push(json!({ "role": role, "parts": parts }));
                }
                _ => {}
            }
        }
        (system, contents)
    }

    fn convert_response(v: &Value, model: &str) -> ChatCompletionResponse {
        let candidates = v.get("candidates").and_then(|c| c.as_array());
        let content: String = candidates
            .and_then(|arr| arr.first())
            .and_then(|c| c.get("content"))
            .and_then(|c| c.get("parts"))
            .and_then(|p| p.as_array())
            .map(|parts| {
                parts.iter()
                    .filter_map(|p| p.get("text").and_then(|t| t.as_str()).map(|s| s.to_string()))
                    .collect::<Vec<_>>()
                    .join("")
            })
            .unwrap_or_default();

        let finish_reason = candidates
            .and_then(|arr| arr.first())
            .and_then(|c| c.get("finishReason"))
            .and_then(|r| r.as_str())
            .map(|s| match s {
                "STOP" => "stop",
                "MAX_TOKENS" => "length",
                "SAFETY" => "content_filter",
                other => other,
            })
            .unwrap_or("stop");

        let usage = v.get("usageMetadata").map(|u| {
            let prompt = u.get("promptTokenCount").and_then(|t| t.as_u64()).unwrap_or(0) as u32;
            let completion = u.get("candidatesTokenCount").and_then(|t| t.as_u64()).unwrap_or(0) as u32;
            Usage {
                prompt_tokens: prompt,
                completion_tokens: completion,
                total_tokens: prompt + completion,
            }
        });

        ChatCompletionResponse {
            id: format!("gemini-{}", chrono::Utc::now().timestamp_millis()),
            object: "chat.completion".into(),
            created: chrono::Utc::now().timestamp(),
            model: model.into(),
            choices: vec![crate::models::chat::Choice {
                index: 0,
                message: Message {
                    role: "assistant".into(),
                    content: Some(MessageContent::Text(content)),
                    tool_calls: None,
                    tool_call_id: None,
                    name: None,
                },
                finish_reason: Some(finish_reason.into()),
            }],
            usage,
            system_fingerprint: None,
        }
    }
}

fn extract_text(content: &Option<MessageContent>) -> String {
    match content {
        Some(MessageContent::Text(t)) => t.clone(),
        Some(MessageContent::Parts(parts)) => parts.iter()
            .filter_map(|p| match p {
                crate::models::chat::ContentPart::Text { text } => Some(text.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(""),
        None => String::new(),
    }
}

#[async_trait]
impl Provider for Gemini {
    fn id(&self) -> ProviderId { ProviderId::Gemini }
    fn is_configured(&self) -> bool { self.api_key.is_some() }

    async fn chat(&self, req: &ChatCompletionRequest) -> AppResult<ChatCompletionResponse> {
        let key = self.key()?;
        let (system, contents) = Self::convert_messages(&req.messages);
        let (_provider_hint, model_name) = req.split_model();
        let model = model_name.to_string();

        let mut body = json!({
            "contents": contents,
            "generationConfig": {
                "temperature": req.temperature,
                "topP": req.top_p,
                "maxOutputTokens": req.max_tokens.unwrap_or(4096),
            }
        });
        if let Some(s) = system {
            body["systemInstruction"] = s;
        }

        let url = format!("{}/models/{}:generateContent", BASE_URL, model);
        let resp = self
            .client
            .post(&url)
            .header("x-goog-api-key", key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::Provider(format!("Gemini request failed: {}", e)))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(map_gemini_error(status, &text));
        }

        let v: Value = resp.json().await
            .map_err(|e| AppError::Provider(format!("Gemini decode failed: {}", e)))?;
        Ok(Self::convert_response(&v, &model))
    }

    async fn chat_stream(
        &self,
        req: &ChatCompletionRequest,
    ) -> AppResult<Box<dyn futures::Stream<Item = StreamEvent> + Send + Unpin>> {
        let key = self.key()?;
        let (system, contents) = Self::convert_messages(&req.messages);
        let (_provider_hint, model_name) = req.split_model();
        let model = model_name.to_string();

        let mut body = json!({
            "contents": contents,
            "generationConfig": {
                "temperature": req.temperature,
                "topP": req.top_p,
                "maxOutputTokens": req.max_tokens.unwrap_or(4096),
            }
        });
        if let Some(s) = system {
            body["systemInstruction"] = s;
        }

        let url = format!("{}/models/{}:streamGenerateContent?alt=sse", BASE_URL, model);
        let resp = self
            .client
            .post(&url)
            .header("x-goog-api-key", key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::Provider(format!("Gemini stream request failed: {}", e)))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(map_gemini_error(status, &text));
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
                            if let Some(event) = parse_gemini_event(&event_str) {
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
            // Stream ended — emit Finish
            let _ = tx.send(StreamEvent::Finish("stop".into())).await;
        });

        Ok(Box::new(ReceiverStream::new(rx)))
    }

    async fn list_models(&self) -> AppResult<Vec<ModelInfo>> {
        Ok(DEFAULT_MODELS.iter().map(|m| ModelInfo {
            id: format!("gemini:{}", m),
            object: "model",
            created: 1_700_000_000,
            owned_by: "google".into(),
        }).collect())
    }
}

/// Parse a Gemini SSE event.
/// Gemini streams chunks of generateContent responses, each with candidates[].content.parts[].text.
fn parse_gemini_event(raw: &str) -> Option<StreamEvent> {
    for line in raw.lines() {
        let line = line.trim();
        if let Some(data) = line.strip_prefix("data: ") {
            let data = data.trim();
            if let Ok(v) = serde_json::from_str::<Value>(data) {
                if let Some(err) = v.get("error") {
                    let msg = err.get("message").and_then(|m| m.as_str()).unwrap_or("gemini error");
                    return Some(StreamEvent::Error(msg.into()));
                }
                let text: String = v.get("candidates")
                    .and_then(|c| c.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|c| c.get("content"))
                    .and_then(|c| c.get("parts"))
                    .and_then(|p| p.as_array())
                    .map(|parts| {
                        parts.iter()
                            .filter_map(|p| p.get("text").and_then(|t| t.as_str()).map(|s| s.to_string()))
                            .collect::<Vec<_>>()
                            .join("")
                    })
                    .unwrap_or_default();
                if !text.is_empty() {
                    return Some(StreamEvent::Delta { content: Some(text), role: None });
                }
                // Usage on final chunk
                if let Some(u) = v.get("usageMetadata") {
                    let prompt = u.get("promptTokenCount").and_then(|t| t.as_u64()).unwrap_or(0) as u32;
                    let completion = u.get("candidatesTokenCount").and_then(|t| t.as_u64()).unwrap_or(0) as u32;
                    return Some(StreamEvent::Usage(Usage {
                        prompt_tokens: prompt,
                        completion_tokens: completion,
                        total_tokens: prompt + completion,
                    }));
                }
            }
        }
    }
    None
}

fn map_gemini_error(status: reqwest::StatusCode, body: &str) -> AppError {
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
        401 | 403 => AppError::Provider(format!("Gemini auth error: {}", msg)),
        429 => AppError::RateLimited(format!("Gemini rate limit: {}", msg)),
        400 | 404 => AppError::BadRequest(format!("Gemini: {}", msg)),
        _ => AppError::Provider(format!("Gemini {} error: {}", status, msg)),
    }
}
