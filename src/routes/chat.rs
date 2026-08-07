//! /v1/chat/completions — OpenAI-compatible chat endpoint.
//!
//! Handles both streaming (SSE) and non-streaming responses. Walks the
//! failover registry to find a working provider.

use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::IntoResponse;
use axum::Json;
use futures::Stream;
use sqlx::SqlitePool;
use std::convert::Infallible;
use std::time::Instant;
use tokio_stream::StreamExt;

use crate::config::Config;
use crate::error::{AppError, AppResult};
use crate::middleware::auth::{record_usage, ApiKeyAuth};
use crate::models::chat::{
    ChatCompletionChunk, ChatCompletionRequest, ChatCompletionResponse, ChunkChoice, Delta, StreamEvent,
};
use crate::models::usage::UsageLog;
use crate::providers::{chat_stream_with_failover, chat_with_failover, SharedRegistry};
use crate::providers::combo;

pub async fn chat_completions(
    State(registry): State<SharedRegistry>,
    State(pool): State<SqlitePool>,
    State(config): State<Config>,
    auth: ApiKeyAuth,
    Json(req): Json<ChatCompletionRequest>,
) -> AppResult<impl IntoResponse> {
    let start = Instant::now();
    let auth_ctx = auth.0;
    let is_stream = req.stream;
    let model_label = req.model.clone();
    let endpoint = "/v1/chat/completions".to_string();

    let registry_guard = registry.read().await;

    // Detect combo requests (model starts with "combo:")
    let combo_spec = combo::ComboSpec::parse(&req.model);

    // Apply compression if enabled (rtk / caveman / all). Opt-in via env var.
    let mut req = req;
    let comp_mode = crate::compression::CompressionMode::from_env();
    if comp_mode != crate::compression::CompressionMode::None {
        let saved = crate::compression::compress(&mut req, comp_mode);
        if saved > 0 {
            tracing::debug!("[compression] saved {} chars", saved);
        }
    }

    // Run guardrails (injection / content filter). Opt-in via env var.
    let guard_mode = crate::guardrails::GuardrailMode::from_env();
    crate::guardrails::check(&req, guard_mode)?;

    // Run plugin before_request hooks (logging, custom transforms, etc.)
    {
        let plugins = crate::plugins::PLUGINS.read().await;
        if let Err(e) = plugins.before_request(&mut req).await {
            let _ = e;
        }
    }

    // Check per-API-key token quota (opt-in via OMNIROUTE_QUOTA_ENABLED)
    crate::plugins::quota::check_quota_for_api_key(&pool, auth_ctx.api_key_id.as_deref()).await?;

    // Check per-org token quota (if the API key belongs to an org)
    // We don't have the org_id readily available here — would need to fetch from api_keys table.
    // For now we pass None; a full impl would query the api_key's org_id first.
    crate::tenant_quota::check(&pool, None).await?;

    // Rate limiting (per-key + per-IP token bucket, in-memory)
    crate::rate_limit::check(auth_ctx.api_key_id.as_deref(), None)?;

    // Response cache lookup (non-streaming only)
    if !is_stream {
        if let Some(cached) = crate::cache::get(&req) {
            tracing::debug!("[cache] hit for model={}", req.model);
            crate::metrics::record_cache_hit();
            return Ok(Json(cached).into_response());
        }
        crate::metrics::record_cache_miss();
    }

    if is_stream {
        let stream: Box<dyn futures::Stream<Item = StreamEvent> + Send + Unpin> = if let Some(spec) = &combo_spec {
            combo::execute_stream(&registry_guard, spec, &req).await?
        } else {
            chat_stream_with_failover(&registry_guard, &req, config.max_retries).await?
        };
        let model = req.model.clone();
        let id = format!("chatcmpl-{}", chrono::Utc::now().timestamp_millis());
        let created = chrono::Utc::now().timestamp();
        let auth_ctx_for_log = auth_ctx.clone();
        let pool_for_log = pool.clone();
        let config_for_log = config.clone();

        // Wrap the provider's stream into SSE events with the OpenAI chunk shape.
        let sse_stream = stream.map(move |event| {
            let chunk = match &event {
                StreamEvent::Delta { content, role } => ChatCompletionChunk {
                    id: id.clone(),
                    object: "chat.completion.chunk",
                    created,
                    model: model.clone(),
                    choices: vec![ChunkChoice {
                        index: 0,
                        delta: Delta {
                            role: role.clone(),
                            content: content.clone(),
                            tool_calls: None,
                        },
                        finish_reason: None,
                    }],
                    usage: None,
                },
                StreamEvent::ToolCallDelta { index, id: tc_id, name, arguments } => {
                    let tc = crate::models::chat::ToolCall {
                        id: tc_id.clone().unwrap_or_default(),
                        call_type: "function".into(),
                        function: crate::models::chat::ToolCallFunction {
                            name: name.clone().unwrap_or_default(),
                            arguments: arguments.clone().unwrap_or_default(),
                        },
                    };
                    ChatCompletionChunk {
                        id: id.clone(),
                        object: "chat.completion.chunk",
                        created,
                        model: model.clone(),
                        choices: vec![ChunkChoice {
                            index: *index,
                            delta: Delta { role: None, content: None, tool_calls: Some(vec![tc]) },
                            finish_reason: None,
                        }],
                        usage: None,
                    }
                }
                StreamEvent::Finish(reason) => ChatCompletionChunk {
                    id: id.clone(),
                    object: "chat.completion.chunk",
                    created,
                    model: model.clone(),
                    choices: vec![ChunkChoice {
                        index: 0,
                        delta: Delta::default(),
                        finish_reason: Some(reason.clone()),
                    }],
                    usage: None,
                },
                StreamEvent::Usage(u) => ChatCompletionChunk {
                    id: id.clone(),
                    object: "chat.completion.chunk",
                    created,
                    model: model.clone(),
                    choices: vec![],
                    usage: Some(u.clone()),
                },
                StreamEvent::Error(msg) => {
                    return Ok::<Event, Infallible>(Event::default().data(
                        serde_json::json!({ "error": { "message": msg, "type": "provider_error" } })
                            .to_string(),
                    ));
                }
            };
            Ok(Event::default().data(serde_json::to_string(&chunk).unwrap_or_else(|_| "{}".into())))
        });

        // After the stream completes, append [DONE] + record usage
        let auth_ctx_done = auth_ctx_for_log.clone();
        let pool_done = pool_for_log.clone();
        let model_label_done = model_label.clone();
        let endpoint_done = endpoint.clone();
        let start_done = start;
        let config_done = config_for_log.clone();
        let sse_stream = sse_stream.chain(futures::stream::once(async move {
            // Best-effort usage log (we don't have token counts for streams unless provider sent them)
            let log = UsageLog {
                id: uuid::Uuid::new_v4().to_string(),
                api_key_id: auth_ctx_done.api_key_id,
                user_id: auth_ctx_done.user_id,
                provider_id: "unknown".into(), // filled in by provider stream metadata if available
                model: model_label_done,
                endpoint: endpoint_done,
                method: "POST".into(),
                status_code: 200,
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
                duration_ms: start_done.elapsed().as_millis() as i64,
                error: None,
                client_ip: None,
                user_agent: None,
                created_at: chrono::Utc::now().to_rfc3339(),
            };
            record_usage(&pool_done, &log).await;
            let _ = config_done;
            Ok::<Event, Infallible>(Event::default().data("[DONE]"))
        }));

        Ok(Sse::new(sse_stream).keep_alive(KeepAlive::default()).into_response())
    } else {
        let mut resp = if let Some(spec) = &combo_spec {
            combo::execute(&registry_guard, spec, &req).await?
        } else {
            chat_with_failover(&registry_guard, &req, config.max_retries).await?
        };

        // Run plugin after_response hooks
        {
            let plugins = crate::plugins::PLUGINS.read().await;
            if let Err(e) = plugins.after_response(&mut resp).await {
                tracing::warn!("[plugins] after_response error: {}", e);
            }
        }

        let duration_ms = start.elapsed().as_millis() as i64;
        let provider_id = resp.system_fingerprint.clone().unwrap_or_else(|| "unknown".into());

        // Compute cost (USD) based on the provider + model pricing table
        let prompt_tokens = resp.usage.as_ref().map(|u| u.prompt_tokens).unwrap_or(0);
        let completion_tokens = resp.usage.as_ref().map(|u| u.completion_tokens).unwrap_or(0);
        let (_provider_hint, model_name) = resp.model.split_once(':')
            .map(|(p, m)| (Some(p), m.to_string()))
            .unwrap_or((None, resp.model.clone()));
        let provider_pid = crate::models::provider::ProviderId::from_str(&provider_id);
        let cost_usd = provider_pid
            .map(|pid| crate::pricing::compute_cost(pid, &model_name, prompt_tokens, completion_tokens))
            .unwrap_or(0.0);

        let log = UsageLog {
            id: uuid::Uuid::new_v4().to_string(),
            api_key_id: auth_ctx.api_key_id,
            user_id: auth_ctx.user_id,
            provider_id: provider_id.clone(),
            model: model_label.clone(),
            endpoint: endpoint.clone(),
            method: "POST".into(),
            status_code: 200,
            prompt_tokens: resp.usage.as_ref().map(|u| u.prompt_tokens as i64).unwrap_or(0),
            completion_tokens: resp.usage.as_ref().map(|u| u.completion_tokens as i64).unwrap_or(0),
            total_tokens: resp.usage.as_ref().map(|u| u.total_tokens as i64).unwrap_or(0),
            duration_ms,
            error: None,
            client_ip: None,
            user_agent: None,
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        // Note: cost_usd would be persisted via a follow-up UPDATE if the column
        // supports it (migration 002 added it). For brevity we log it:
        tracing::info!(
            "[usage] provider={} model={} tokens={}/{}/{} cost=${:.6} duration={}ms",
            log.provider_id, log.model, log.prompt_tokens, log.completion_tokens, log.total_tokens,
            cost_usd, log.duration_ms
        );
        record_usage(&pool, &log).await;

        // Fire webhook + live WS broadcast
        let usage_clone = log.clone();
        crate::webhooks::fire_usage(crate::webhooks::UsageEvent {
            api_key_id: usage_clone.api_key_id,
            user_id: usage_clone.user_id,
            provider_id: usage_clone.provider_id.clone(),
            model: usage_clone.model.clone(),
            prompt_tokens: usage_clone.prompt_tokens as u32,
            completion_tokens: usage_clone.completion_tokens as u32,
            total_tokens: usage_clone.total_tokens as u32,
            duration_ms: usage_clone.duration_ms as u64,
            status_code: usage_clone.status_code as u16,
        });
        crate::live::broadcast_usage(
            &usage_clone.provider_id,
            &usage_clone.model,
            usage_clone.prompt_tokens as u32,
            usage_clone.completion_tokens as u32,
            usage_clone.duration_ms as u64,
            usage_clone.status_code as u16,
        );

        // Run plugin on_usage hooks
        {
            let plugins = crate::plugins::PLUGINS.read().await;
            plugins.on_usage(&log).await;
        }

        // Store response in cache (non-streaming only)
        crate::cache::set(&req, resp.clone());

        // Record Prometheus metrics
        crate::metrics::record_request(&log.provider_id, log.status_code as u16);
        crate::metrics::record_tokens(&log.provider_id, log.prompt_tokens as u32, log.completion_tokens as u32);
        crate::metrics::record_cost(&log.provider_id, cost_usd);

        // Increment org quota usage (if applicable)
        // (Passing None for now since we don't have org_id — see comment above)
        crate::tenant_quota::increment(&pool, None, log.total_tokens as u32).await;

        Ok(Json(resp).into_response())
    }
}
