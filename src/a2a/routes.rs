//! A2A HTTP routes — agent discovery + invocation.

use axum::extract::{Path, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::SqlitePool;

use crate::error::{AppError, AppResult};
use crate::middleware::auth::ApiKeyAuth;
use crate::models::chat::{ChatCompletionRequest, Message, MessageContent};
use crate::providers::SharedRegistry;
use super::agents;

#[derive(Debug, Serialize)]
pub struct AgentListResponse {
    pub agents: Vec<agents::Agent>,
}

pub async fn list_agents() -> AppResult<Json<Value>> {
    Ok(Json(json!({ "agents": agents::list().await })))
}

pub async fn get_agent(Path(id): Path<String>) -> AppResult<Json<Value>> {
    let agent = agents::get(&id).await
        .ok_or_else(|| AppError::NotFound(format!("agent not found: {}", id)))?;
    Ok(Json(json!(agent)))
}

#[derive(Debug, Deserialize)]
pub struct InvokeAgentRequest {
    /// The user's input — wrapped as a single user message.
    pub input: String,
    /// Optional: override the agent's model.
    #[serde(default)]
    pub model: Option<String>,
    /// Optional: override the agent's temperature.
    #[serde(default)]
    pub temperature: Option<f32>,
    /// Optional: additional system instructions prepended.
    #[serde(default)]
    pub system_prefix: Option<String>,
    /// Optional: stream the response (default false).
    #[serde(default)]
    pub stream: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct InvokeAgentResponse {
    pub agent_id: String,
    pub output: String,
    pub model_used: String,
    pub usage: Option<crate::models::chat::Usage>,
}

pub async fn invoke_agent(
    State(registry): State<SharedRegistry>,
    State(_pool): State<SqlitePool>,
    _auth: ApiKeyAuth,
    Path(id): Path<String>,
    Json(req): Json<InvokeAgentRequest>,
) -> AppResult<Json<Value>> {
    let agent = agents::get(&id).await
        .ok_or_else(|| AppError::NotFound(format!("agent not found: {}", id)))?;

    let model = req.model.unwrap_or_else(|| agent.model.clone());
    let temperature = req.temperature.unwrap_or(agent.temperature) as f64;

    let mut messages = Vec::with_capacity(2);
    let system = if let Some(prefix) = &req.system_prefix {
        format!("{}\n\n{}", prefix, agent.system_prompt)
    } else {
        agent.system_prompt.clone()
    };
    messages.push(Message {
        role: "system".into(),
        content: Some(MessageContent::Text(system)),
        tool_calls: None, tool_call_id: None, name: None,
    });
    messages.push(Message {
        role: "user".into(),
        content: Some(MessageContent::Text(req.input)),
        tool_calls: None, tool_call_id: None, name: None,
    });

    let chat_req = ChatCompletionRequest {
        model,
        messages,
        temperature,
        top_p: 1.0,
        max_tokens: Some(agent.max_tokens),
        stream: false,
        stop: None, seed: None, tools: None, tool_choice: None,
        extra: serde_json::Map::new(),
    };

    let registry_guard = registry.read().await;
    let resp = if let Some(spec) = crate::providers::combo::ComboSpec::parse(&chat_req.model) {
        crate::providers::combo::execute(&registry_guard, &spec, &chat_req).await?
    } else {
        crate::providers::chat_with_failover(&registry_guard, &chat_req, 1).await?
    };

    let output = resp.choices.first()
        .and_then(|c| c.message.content.as_ref())
        .map(|c| match c {
            MessageContent::Text(t) => t.clone(),
            _ => "[non-text response]".into(),
        })
        .unwrap_or_default();

    Ok(Json(json!({
        "agent_id": agent.id,
        "output": output,
        "model_used": resp.model,
        "usage": resp.usage,
    })))
}

/// Register a new agent dynamically (POST /v1/a2a/agents).
pub async fn register_agent(
    _auth: ApiKeyAuth,
    Json(agent): Json<agents::Agent>,
) -> AppResult<Json<Value>> {
    if agent.id.is_empty() {
        return Err(AppError::BadRequest("agent id is required".into()));
    }
    agents::register(agent.clone()).await;
    Ok(Json(json!({ "success": true, "agent": agent })))
}

/// Delete an agent (DELETE /v1/a2a/agents/:id).
pub async fn delete_agent(
    _auth: ApiKeyAuth,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    let ok = agents::unregister(&id).await;
    Ok(Json(json!({ "success": ok })))
}
