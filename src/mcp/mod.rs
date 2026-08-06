//! MCP (Model Context Protocol) server — exposes OmniRoute as a tool source
//! for Claude Desktop, Cursor, Continue, and other MCP-aware clients.
//!
//! Spec: https://modelcontextprotocol.io/specification
//!
//! Two transports:
//!   - SSE  at /v1/mcp/sse           (HTTP-based, for browser clients)
//!   - stdio via `omniroute mcp`     (for Claude Desktop / native clients)
//!
//! Tools exposed:
//!   - `omniroute_chat`     — chat completion via the failover registry
//!   - `omniroute_list_models` — list available models
//!   - `omniroute_combo`    — run a combo strategy across multiple providers
//!   - `omniroute_usage`    — query usage stats

pub mod tools;
pub mod transport;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Value,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl JsonRpcResponse {
    pub fn success(id: Value, result: Value) -> Self {
        Self { jsonrpc: "2.0".into(), id, result: Some(result), error: None }
    }
    pub fn error(id: Value, code: i32, message: impl Into<String>) -> Self {
        Self { jsonrpc: "2.0".into(), id, result: None, error: Some(JsonRpcError {
            code, message: message.into(), data: None,
        }) }
    }
}

// ─── MCP protocol types ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct McpToolCallResult {
    pub content: Vec<McpContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum McpContent {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { data: String, #[serde(rename = "mimeType")] mime_type: String },
}

/// Handle a single JSON-RPC request and return a response.
pub async fn handle_request(
    req: &JsonRpcRequest,
    registry: &crate::providers::Registry,
    pool: &sqlx::SqlitePool,
) -> JsonRpcResponse {
    match req.method.as_str() {
        "initialize" => JsonRpcResponse::success(req.id.clone(), serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {},
                "resources": {},
                "prompts": {},
            },
            "serverInfo": {
                "name": "omniroute",
                "version": env!("CARGO_PKG_VERSION"),
            }
        })),
        "initialized" => JsonRpcResponse::success(req.id.clone(), Value::Null),
        "tools/list" => {
            let tools = tools::list_all();
            JsonRpcResponse::success(req.id.clone(), serde_json::json!({ "tools": tools }))
        }
        "tools/call" => {
            let params: tools::ToolCallParams = match serde_json::from_value(req.params.clone()) {
                Ok(p) => p,
                Err(e) => return JsonRpcResponse::error(req.id.clone(), -32602, format!("invalid params: {}", e)),
            };
            match tools::call(&params.name, &params.arguments, registry, pool).await {
                Ok(result) => JsonRpcResponse::success(req.id.clone(), serde_json::to_value(result).unwrap_or(Value::Null)),
                Err(e) => JsonRpcResponse::error(req.id.clone(), -32603, e),
            }
        }
        "resources/list" => JsonRpcResponse::success(req.id.clone(), serde_json::json!({ "resources": [] })),
        "prompts/list" => JsonRpcResponse::success(req.id.clone(), serde_json::json!({ "prompts": [] })),
        _ => JsonRpcResponse::error(req.id.clone(), -32601, format!("method not found: {}", req.method)),
    }
}
