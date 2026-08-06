//! /v1/audio/speech — OpenAI-compatible text-to-speech.
//! Forwards to OpenAI's TTS endpoint. Provider routing is hardcoded to OpenAI
//! for now (only OpenAI offers this today; other providers can be added).

use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;
use crate::error::{AppError, AppResult};
use crate::middleware::auth::ApiKeyAuth;

#[derive(Debug, Deserialize)]
pub struct SpeechRequest {
    pub model: String,        // "tts-1" | "tts-1-hd"
    pub input: String,        // text to speak
    pub voice: String,        // "alloy" | "echo" | "fable" | "onyx" | "nova" | "shimmer"
    #[serde(default = "default_format")]
    pub response_format: String, // "mp3" | "opus" | "aac" | "flac" | "wav"
    #[serde(default = "default_speed")]
    pub speed: f32,
}

fn default_format() -> String { "mp3".into() }
fn default_speed() -> f32 { 1.0 }

pub async fn create_speech(
    _auth: ApiKeyAuth,
    Json(req): Json<SpeechRequest>,
) -> AppResult<impl IntoResponse> {
    let api_key = std::env::var("OPENAI_API_KEY")
        .map_err(|_| AppError::Provider("OPENAI_API_KEY not configured for TTS".into()))?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build().map_err(|e| AppError::Internal(e.to_string()))?;

    let body = serde_json::json!({
        "model": req.model,
        "input": req.input,
        "voice": req.voice,
        "response_format": req.response_format,
        "speed": req.speed,
    });

    let resp = client.post("https://api.openai.com/v1/audio/speech")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send().await
        .map_err(|e| AppError::Provider(format!("TTS request failed: {}", e)))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(AppError::Provider(format!("TTS {} error: {}", status, text)));
    }

    let bytes = resp.bytes().await
        .map_err(|e| AppError::Provider(format!("TTS body read failed: {}", e)))?;

    let mime = match req.response_format.as_str() {
        "mp3" => "audio/mpeg",
        "opus" => "audio/opus",
        "aac" => "audio/aac",
        "flac" => "audio/flac",
        "wav" => "audio/wav",
        _ => "application/octet-stream",
    };

    Ok((
        [(axum::http::header::CONTENT_TYPE, mime)],
        bytes,
    ))
}
