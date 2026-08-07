//! OpenAI-compatible chat completion types.
//!
//! Reference: https://platform.openai.com/docs/api-reference/chat
//!
//! These types are the contract with every API client (Cursor, Cline, Codex,
//! Continue, etc.) — they MUST match the OpenAI shape exactly.

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ─── Request ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChatCompletionRequest {
    /// ID of the model to use. May be `<provider>:<model>` (e.g. "openai:gpt-4o")
    /// or just `<model>` (e.g. "gpt-4o") — in the latter case the failover
    /// registry picks the first provider that has it.
    pub model: String,

    /// The messages so far, in chronological order.
    pub messages: Vec<Message>,

    /// What sampling temperature to use, 0–2. Default 1.0.
    #[serde(default = "default_temperature")]
    pub temperature: f64,

    /// Nucleus sampling: 0–1. Default 1.0.
    #[serde(default = "default_top_p")]
    pub top_p: f64,

    /// Max tokens to generate. None = provider default.
    #[serde(default)]
    pub max_tokens: Option<u32>,

    /// Whether to stream tokens back as SSE. Default false.
    #[serde(default)]
    pub stream: bool,

    /// Stop sequences (up to 4). Generation halts on any match.
    #[serde(default)]
    pub stop: Option<StopSequence>,

    /// Random seed for deterministic output (provider-dependent).
    #[serde(default)]
    pub seed: Option<i64>,

    /// List of tools the model may call.
    #[serde(default)]
    pub tools: Option<Vec<Tool>>,

    /// "auto" | "none" | "required" | {"type":"function","function":{"name":"..."}}
    #[serde(default)]
    pub tool_choice: Option<Value>,

    /// Allow arbitrary extra fields (provider-specific knobs).
    #[serde(flatten)]
    pub extra: serde_json::Map<String, Value>,
}

fn default_temperature() -> f64 { 1.0 }
fn default_top_p() -> f64 { 1.0 }

impl ChatCompletionRequest {
    /// Strip `provider:` prefix if present, returning (provider_id, model_name).
    pub fn split_model(&self) -> (Option<&str>, &str) {
        match self.model.split_once(':') {
            Some((p, m)) => (Some(p), m),
            None => (None, self.model.as_str()),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum StopSequence {
    Single(String),
    Multiple(Vec<String>),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Message {
    /// "system" | "user" | "assistant" | "tool"
    pub role: String,

    /// The message content. May be a string OR an array of content parts
    /// (vision: image_url, etc.).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<MessageContent>,

    /// For assistant messages that called a tool: the tool call object.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,

    /// For tool result messages: the id of the tool call this is responding to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,

    /// Optional name (for some providers).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Parts(Vec<ContentPart>),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum ContentPart {
    #[serde(rename = "text")]
    Text { text: String },

    #[serde(rename = "image_url")]
    ImageUrl { image_url: ImageUrl },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ImageUrl {
    /// URL or data: URL (base64).
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Tool {
    #[serde(rename = "type")]
    pub tool_type: String,  // always "function" today
    pub function: ToolFunction,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolFunction {
    pub name: String,
    pub description: Option<String>,
    pub parameters: Option<Value>, // JSON Schema
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,  // "function"
    pub function: ToolCallFunction,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: String,  // JSON string
}

// ─── Non-streaming response ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionResponse {
    pub id: String,
    #[serde(default)]
    pub object: &'static str,           // "chat.completion"
    pub created: i64,
    pub model: String,
    #[serde(default)]
    pub choices: Vec<Choice>,
    #[serde(default)]
    pub usage: Option<Usage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_fingerprint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Choice {
    pub index: u32,
    pub message: Message,
    pub finish_reason: Option<String>, // "stop" | "length" | "tool_calls" | null
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

// ─── Streaming chunk (SSE) ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct ChatCompletionChunk {
    pub id: String,
    pub object: &'static str,           // "chat.completion.chunk"
    pub created: i64,
    pub model: String,
    pub choices: Vec<ChunkChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChunkChoice {
    pub index: u32,
    pub delta: Delta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Delta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
}

/// Internal normalized streaming event — providers convert their native
/// stream format to this, then the route handler converts to OpenAI SSE.
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// Emit a content delta.
    Delta { content: Option<String>, role: Option<String> },
    /// Emit a tool call delta.
    ToolCallDelta { index: u32, id: Option<String>, name: Option<String>, arguments: Option<String> },
    /// End of stream with a finish reason ("stop" | "length" | "tool_calls").
    Finish(String),
    /// Usage stats (some providers send these at the end).
    Usage(Usage),
    /// Terminal error (provider returned an error mid-stream).
    Error(String),
}
