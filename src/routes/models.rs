//! /v1/models — list all available models from all configured providers.

use axum::extract::State;
use axum::Json;
use serde_json::{json, Value};

use crate::error::AppResult;
use crate::middleware::auth::ApiKeyAuth;
use crate::providers::{list_all_models, SharedRegistry};

pub async fn list_models(
    State(registry): State<SharedRegistry>,
    _auth: ApiKeyAuth,
) -> AppResult<Json<Value>> {
    let registry = registry.read().await;
    let models = list_all_models(&registry).await;
    Ok(Json(json!({
        "object": "list",
        "data": models,
    })))
}
