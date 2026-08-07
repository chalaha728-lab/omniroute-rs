//! /v1/embeddings — OpenAI-compatible text embeddings.
//! Forwards to OpenAI's embeddings endpoint.

use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::SqlitePool;

use crate::error::{AppError, AppResult};
use crate::middleware::auth::ApiKeyAuth;

#[derive(Debug, Deserialize)]
pub struct EmbeddingRequest {
    pub model: String,
    pub input: EmbeddingInput,
    #[serde(default)]
    pub encoding_format: Option<String>,
    #[serde(default)]
    pub dimensions: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum EmbeddingInput {
    Single(String),
    Multiple(Vec<String>),
}

#[derive(Debug, Serialize)]
pub struct EmbeddingResponse {
    pub object: &'static str,
    pub data: Vec<EmbeddingData>,
    pub model: String,
    pub usage: EmbeddingUsage,
}

#[derive(Debug, Serialize)]
pub struct EmbeddingData {
    pub object: &'static str,
    pub index: u32,
    pub embedding: Vec<f32>,
}

#[derive(Debug, Serialize)]
pub struct EmbeddingUsage {
    pub prompt_tokens: u32,
    pub total_tokens: u32,
}

pub async fn create_embedding(
    State(_pool): State<SqlitePool>,
    _auth: ApiKeyAuth,
    Json(req): Json<EmbeddingRequest>,
) -> AppResult<Json<Value>> {
    let api_key = std::env::var("OPENAI_API_KEY")
        .map_err(|_| AppError::Provider("OPENAI_API_KEY not configured for embeddings".into()))?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build().map_err(|e| AppError::Internal(e.to_string()))?;

    let model_name = req.model.split_once(':').map(|(_, m)| m).unwrap_or(&req.model);

    let input_value: Value = match &req.input {
        EmbeddingInput::Single(s) => Value::String(s.clone()),
        EmbeddingInput::Multiple(arr) => serde_json::to_value(arr).unwrap_or(Value::Null),
    };

    let body = json!({
        "model": model_name,
        "input": input_value,
        "encoding_format": req.encoding_format.as_deref().unwrap_or("float"),
        "dimensions": req.dimensions,
    });

    let resp = client.post("https://api.openai.com/v1/embeddings")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send().await
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
