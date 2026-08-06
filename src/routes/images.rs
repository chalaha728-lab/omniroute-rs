//! /v1/images/generations — OpenAI-compatible image generation.
//! Forwards to OpenAI's DALL-E endpoint.

use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::error::{AppError, AppResult};
use crate::middleware::auth::ApiKeyAuth;

#[derive(Debug, Deserialize)]
pub struct ImageRequest {
    pub model: String,           // "dall-e-3" | "dall-e-2"
    pub prompt: String,
    #[serde(default)]
    pub n: Option<u32>,           // 1-10
    #[serde(default)]
    pub size: Option<String>,     // "256x256" | "512x512" | "1024x1024" | "1792x1024" | "1024x1792"
    #[serde(default)]
    pub quality: Option<String>,  // "standard" | "hd"
    #[serde(default)]
    pub style: Option<String>,    // "vivid" | "natural"
    #[serde(default)]
    pub response_format: Option<String>, // "url" (default) | "b64_json"
}

pub async fn create_image(
    _auth: ApiKeyAuth,
    Json(req): Json<ImageRequest>,
) -> AppResult<Json<Value>> {
    let api_key = std::env::var("OPENAI_API_KEY")
        .map_err(|_| AppError::Provider("OPENAI_API_KEY not configured for image gen".into()))?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build().map_err(|e| AppError::Internal(e.to_string()))?;

    let body = serde_json::json!({
        "model": req.model,
        "prompt": req.prompt,
        "n": req.n.unwrap_or(1),
        "size": req.size.as_deref().unwrap_or("1024x1024"),
        "quality": req.quality.as_deref().unwrap_or("standard"),
        "style": req.style.as_deref().unwrap_or("vivid"),
        "response_format": req.response_format.as_deref().unwrap_or("url"),
    });

    let resp = client.post("https://api.openai.com/v1/images/generations")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send().await
        .map_err(|e| AppError::Provider(format!("image gen request failed: {}", e)))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(AppError::Provider(format!("image gen {} error: {}", status, text)));
    }

    let v: Value = resp.json().await
        .map_err(|e| AppError::Provider(format!("image gen decode failed: {}", e)))?;
    Ok(Json(v))
}
