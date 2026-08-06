//! MCP tools — the actual tool implementations.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::SqlitePool;

use crate::error::AppResult;
use crate::providers::Registry;
use super::{McpContent, McpTool, McpToolCallResult};

#[derive(Debug, Clone, Deserialize)]
pub struct ToolCallParams {
    pub name: String,
    #[serde(default)]
    pub arguments: Value,
}

/// List all tools this MCP server exposes.
pub fn list_all() -> Vec<McpTool> {
    vec![
        McpTool {
            name: "omniroute_chat".into(),
            description: "Send a chat completion request to OmniRoute. The model field accepts either '<provider>:<model>' (e.g. 'openai:gpt-4o') or just '<model>' (uses failover).".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "model": { "type": "string", "description": "Model id, e.g. 'openai:gpt-4o' or 'anthropic:claude-3-5-sonnet-20241022'" },
                    "messages": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "role": { "type": "string", "enum": ["system", "user", "assistant"] },
                                "content": { "type": "string" }
                            },
                            "required": ["role", "content"]
                        }
                    },
                    "temperature": { "type": "number", "default": 1.0 },
                    "max_tokens": { "type": "integer" },
                },
                "required": ["model", "messages"]
            }),
        },
        McpTool {
            name: "omniroute_list_models".into(),
            description: "List all available models across configured providers.".into(),
            input_schema: json!({ "type": "object", "properties": {} }),
        },
        McpTool {
            name: "omniroute_combo".into(),
            description: "Run a combo strategy across multiple providers. Model format: 'combo:<strategy>:<targets>' where strategy is one of race, parallel, sequential, firstsuccess, majorityvote. Targets are comma-separated 'provider:model' pairs.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "model": { "type": "string", "description": "e.g. 'combo:race:openai:gpt-4o,anthropic:claude-3-5-sonnet-20241022'" },
                    "messages": { "type": "array" }
                },
                "required": ["model", "messages"]
            }),
        },
        McpTool {
            name: "omniroute_usage".into(),
            description: "Query usage statistics: total requests, tokens, errors, broken down by provider.".into(),
            input_schema: json!({ "type": "object", "properties": {} }),
        },
    ]
}

/// Execute a tool call by name.
pub async fn call(
    name: &str,
    args: &Value,
    registry: &Registry,
    pool: &SqlitePool,
) -> Result<McpToolCallResult, String> {
    match name {
        "omniroute_chat" => call_chat(args, registry).await,
        "omniroute_list_models" => call_list_models(registry).await,
        "omniroute_combo" => call_chat(args, registry).await, // combo uses same chat path with combo: model
        "omniroute_usage" => call_usage(pool).await,
        _ => Err(format!("unknown tool: {}", name)),
    }
}

async fn call_chat(args: &Value, registry: &Registry) -> Result<McpToolCallResult, String> {
    let req: crate::models::chat::ChatCompletionRequest =
        serde_json::from_value(args.clone()).map_err(|e| format!("invalid chat request: {}", e))?;
    let resp = crate::providers::chat_with_failover(registry, &req, 1).await
        .map_err(|e| e.to_string())?;
    let text = resp.choices.first()
        .and_then(|c| c.message.content.as_ref())
        .map(|c| match c {
            crate::models::chat::MessageContent::Text(t) => t.clone(),
            _ => "[non-text content]".into(),
        })
        .unwrap_or_else(|| "[no response]".into());
    Ok(McpToolCallResult {
        content: vec![McpContent::Text { text }],
        is_error: Some(false),
    })
}

async fn call_list_models(registry: &Registry) -> Result<McpToolCallResult, String> {
    let models = crate::providers::list_all_models(registry).await;
    let text = serde_json::to_string_pretty(&models).map_err(|e| e.to_string())?;
    Ok(McpToolCallResult {
        content: vec![McpContent::Text { text }],
        is_error: Some(false),
    })
}

async fn call_usage(pool: &SqlitePool) -> Result<McpToolCallResult, String> {
    let total_requests: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM usage_logs")
        .fetch_one(pool).await.map_err(|e| e.to_string())?;
    let total_tokens: i64 = sqlx::query_scalar("SELECT COALESCE(SUM(total_tokens), 0) FROM usage_logs")
        .fetch_one(pool).await.map_err(|e| e.to_string())?;
    let error_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM usage_logs WHERE status_code >= 400")
        .fetch_one(pool).await.map_err(|e| e.to_string())?;

    let by_provider: Vec<(String, i64, i64)> = sqlx::query_as(
        "SELECT provider_id, COUNT(*), COALESCE(SUM(total_tokens), 0) FROM usage_logs GROUP BY provider_id"
    ).fetch_all(pool).await.map_err(|e| e.to_string())?;

    let mut text = format!("Total requests: {}\nTotal tokens: {}\nErrors: {}\n\nBy provider:\n",
                            total_requests, total_tokens, error_count);
    for (pid, reqs, tokens) in by_provider {
        text.push_str(&format!("  {}: {} requests, {} tokens\n", pid, reqs, tokens));
    }
    Ok(McpToolCallResult {
        content: vec![McpContent::Text { text }],
        is_error: Some(false),
    })
}
