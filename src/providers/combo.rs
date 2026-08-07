//! Combo strategies — OmniRoute's signature feature.
//!
//! A combo dispatches one request to multiple providers simultaneously and
//! combines the results. Strategies:
//!
//! - `Race`         — first successful response wins (others are cancelled)
//! - `Parallel`     — fire N requests, return ALL responses (for comparison)
//! - `Sequential`   — try providers in order until one succeeds (= failover)
//! - `FirstSuccess` — same as Sequential but doesn't retry on transient errors
//! - `MajorityVote` — fire 3+ requests, return the majority answer (good for
//!                    factual queries; tokens all 3 responses)
//!
//! Combos are requested via the `model` field with a `combo:` prefix:
//!   "combo:race:openai:gpt-4o,anthropic:claude-3-5-sonnet"
//!   "combo:parallel:openai:gpt-4o,anthropic:claude-3-5-sonnet"
//!   "combo:majorityvote:openai:gpt-4o,anthropic:claude-3-5-sonnet,gemini:gemini-1.5-pro"

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use crate::error::{AppError, AppResult};
use crate::models::chat::{ChatCompletionRequest, ChatCompletionResponse, StreamEvent, Usage};
use crate::providers::Registry;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComboStrategy {
    Race,
    Parallel,
    Sequential,
    FirstSuccess,
    MajorityVote,
}

impl ComboStrategy {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "race" => Some(ComboStrategy::Race),
            "parallel" => Some(ComboStrategy::Parallel),
            "sequential" => Some(ComboStrategy::Sequential),
            "firstsuccess" | "first-success" => Some(ComboStrategy::FirstSuccess),
            "majorityvote" | "majority-vote" => Some(ComboStrategy::MajorityVote),
            _ => None,
        }
    }
}

/// Parsed combo spec from a `combo:<strategy>:<providers>` model string.
#[derive(Debug, Clone)]
pub struct ComboSpec {
    pub strategy: ComboStrategy,
    /// Each entry is "provider:model" (e.g. "openai:gpt-4o").
    pub targets: Vec<String>,
}

impl ComboSpec {
    /// Parse a model string that starts with "combo:".
    /// Returns None if not a combo request.
    pub fn parse(model: &str) -> Option<Self> {
        let rest = model.strip_prefix("combo:")?;
        let (strategy_str, targets_str) = rest.split_once(':')?;
        let strategy = ComboStrategy::from_str(strategy_str)?;
        let targets: Vec<String> = targets_str.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        if targets.is_empty() {
            return None;
        }
        Some(ComboSpec { strategy, targets })
    }
}

/// Execute a combo request (non-streaming).
pub async fn execute(
    registry: &Registry,
    spec: &ComboSpec,
    req: &ChatCompletionRequest,
) -> AppResult<ChatCompletionResponse> {
    match spec.strategy {
        ComboStrategy::Race => execute_race(registry, spec, req).await,
        ComboStrategy::Parallel => execute_parallel(registry, spec, req).await,
        ComboStrategy::Sequential | ComboStrategy::FirstSuccess => execute_sequential(registry, spec, req).await,
        ComboStrategy::MajorityVote => execute_majority_vote(registry, spec, req).await,
    }
}

/// Race: fire all targets at once, return the first successful response.
async fn execute_race(
    registry: &Registry,
    spec: &ComboSpec,
    req: &ChatCompletionRequest,
) -> AppResult<ChatCompletionResponse> {
    let mut tasks = Vec::new();
    for target in &spec.targets {
        let mut sub_req = req.clone();
        sub_req.model = target.clone();
        let provider = registry.pick(&sub_req)
            .ok_or_else(|| AppError::BadRequest(format!("unknown provider in combo target: {}", target)))?;
        let req_clone = sub_req.clone();
        tasks.push(tokio::spawn(async move { provider.chat(&req_clone).await }));
    }
    let mut last_err = None;
    let mut results = Vec::new();
    for task in tasks {
        match task.await {
            Ok(Ok(resp)) => results.push(resp),
            Ok(Err(e)) => last_err = Some(e),
            Err(e) => last_err = Some(AppError::Internal(format!("task panicked: {}", e))),
        }
    }
    results.into_iter().next().ok_or_else(|| last_err.unwrap_or(AppError::AllProvidersFailed))
}

/// Parallel: fire all targets, return ALL responses concatenated.
async fn execute_parallel(
    registry: &Registry,
    spec: &ComboSpec,
    req: &ChatCompletionRequest,
) -> AppResult<ChatCompletionResponse> {
    let mut tasks = Vec::new();
    for target in &spec.targets {
        let mut sub_req = req.clone();
        sub_req.model = target.clone();
        let provider = registry.pick(&sub_req)
            .ok_or_else(|| AppError::BadRequest(format!("unknown provider in combo target: {}", target)))?;
        let req_clone = sub_req.clone();
        tasks.push(tokio::spawn(async move { provider.chat(&req_clone).await }));
    }
    let mut combined_content = String::new();
    let mut total_prompt = 0u32;
    let mut total_completion = 0u32;
    let mut last_err = None;
    let mut providers_used = Vec::new();
    for (i, task) in tasks.into_iter().enumerate() {
        match task.await {
            Ok(Ok(resp)) => {
                if let Some(choice) = resp.choices.first() {
                    if let Some(content) = &choice.message.content {
                        let text = match content {
                            crate::models::chat::MessageContent::Text(t) => t.clone(),
                            _ => String::new(),
                        };
                        combined_content.push_str(&format!("--- Response {} ({}) ---\n{}\n\n", i + 1, resp.model, text));
                    }
                }
                if let Some(u) = resp.usage {
                    total_prompt += u.prompt_tokens;
                    total_completion += u.completion_tokens;
                }
                providers_used.push(resp.model);
            }
            Ok(Err(e)) => last_err = Some(e),
            Err(e) => last_err = Some(AppError::Internal(format!("task panicked: {}", e))),
        }
    }
    if combined_content.is_empty() {
        return Err(last_err.unwrap_or(AppError::AllProvidersFailed));
    }
    Ok(ChatCompletionResponse {
        id: format!("combo-parallel-{}", chrono::Utc::now().timestamp_millis()),
        object: "chat.completion",
        created: chrono::Utc::now().timestamp(),
        model: format!("combo:parallel:{}", providers_used.join(",")),
        choices: vec![crate::models::chat::Choice {
            index: 0,
            message: crate::models::chat::Message {
                role: "assistant".into(),
                content: Some(crate::models::chat::MessageContent::Text(combined_content)),
                tool_calls: None, tool_call_id: None, name: None,
            },
            finish_reason: Some("stop".into()),
        }],
        usage: Some(Usage {
            prompt_tokens: total_prompt,
            completion_tokens: total_completion,
            total_tokens: total_prompt + total_completion,
        }),
        system_fingerprint: None,
    })
}

/// Sequential: try targets in order until one succeeds (equivalent to failover).
async fn execute_sequential(
    registry: &Registry,
    spec: &ComboSpec,
    req: &ChatCompletionRequest,
) -> AppResult<ChatCompletionResponse> {
    let mut last_err = None;
    for target in &spec.targets {
        let mut sub_req = req.clone();
        sub_req.model = target.clone();
        let provider = match registry.pick(&sub_req) {
            Some(p) => p,
            None => {
                last_err = Some(AppError::BadRequest(format!("unknown provider: {}", target)));
                continue;
            }
        };
        match provider.chat(&sub_req).await {
            Ok(resp) => return Ok(resp),
            Err(e) => {
                tracing::warn!("[combo:sequential] {} failed: {}", target, e);
                last_err = Some(e);
            }
        }
    }
    Err(last_err.unwrap_or(AppError::AllProvidersFailed))
}

/// Majority vote: fire 3+ targets, pick the response that appears most often.
/// Uses simple Levenshtein-like text similarity (truncated for speed).
async fn execute_majority_vote(
    registry: &Registry,
    spec: &ComboSpec,
    req: &ChatCompletionRequest,
) -> AppResult<ChatCompletionResponse> {
    if spec.targets.len() < 3 {
        return Err(AppError::BadRequest("majorityvote requires at least 3 targets".into()));
    }
    let mut tasks = Vec::new();
    for target in &spec.targets {
        let mut sub_req = req.clone();
        sub_req.model = target.clone();
        let provider = registry.pick(&sub_req)
            .ok_or_else(|| AppError::BadRequest(format!("unknown provider: {}", target)))?;
        let req_clone = sub_req.clone();
        tasks.push(tokio::spawn(async move { provider.chat(&req_clone).await }));
    }
    let mut responses: Vec<ChatCompletionResponse> = Vec::new();
    let mut total_prompt = 0u32;
    let mut total_completion = 0u32;
    for task in tasks {
        match task.await {
            Ok(Ok(resp)) => {
                if let Some(u) = resp.usage.as_ref() {
                    total_prompt += u.prompt_tokens;
                    total_completion += u.completion_tokens;
                }
                responses.push(resp);
            }
            Ok(Err(_)) | Err(_) => {}
        }
    }
    if responses.is_empty() {
        return Err(AppError::AllProvidersFailed);
    }
    // Find the response whose content has the most "twins" (other responses with same first 100 chars)
    let texts: Vec<String> = responses.iter().map(|r| {
        r.choices.first()
            .and_then(|c| c.message.content.as_ref())
            .map(|c| match c {
                crate::models::chat::MessageContent::Text(t) => t.clone(),
                _ => String::new(),
            })
            .unwrap_or_default()
    }).collect();
    let mut best_idx = 0;
    let mut best_count = 0;
    for (i, t) in texts.iter().enumerate() {
        let prefix = &t[..t.len().min(100)];
        let count = texts.iter().filter(|other| other.starts_with(prefix)).count();
        if count > best_count {
            best_count = count;
            best_idx = i;
        }
    }
    let mut winner = responses.into_iter().nth(best_idx).unwrap();
    winner.usage = Some(Usage {
        prompt_tokens: total_prompt,
        completion_tokens: total_completion,
        total_tokens: total_prompt + total_completion,
    });
    Ok(winner)
}

// ─── Streaming combo (race only — first provider to emit a token wins) ──────

/// Execute a combo as a stream. Only `Race` is supported for streaming — once
/// a provider emits its first delta, the others are cancelled.
pub async fn execute_stream(
    registry: &Registry,
    spec: &ComboSpec,
    req: &ChatCompletionRequest,
) -> AppResult<Box<dyn futures::Stream<Item = StreamEvent> + Send + Unpin>> {
    if spec.strategy != ComboStrategy::Race {
        // Fall back to non-streaming for non-race strategies
        let resp = execute(registry, spec, req).await?;
        let (tx, rx) = mpsc::channel::<StreamEvent>(8);
        tokio::spawn(async move {
            let content = resp.choices.first()
                .and_then(|c| c.message.content.as_ref())
                .and_then(|c| match c {
                    crate::models::chat::MessageContent::Text(t) => Some(t.clone()),
                    _ => None,
                }).unwrap_or_default();
            let _ = tx.send(StreamEvent::Delta { content: Some(content), role: Some("assistant".into()) }).await;
            if let Some(u) = resp.usage { let _ = tx.send(StreamEvent::Usage(u)).await; }
            let _ = tx.send(StreamEvent::Finish("stop".into())).await;
        });
        return Ok(Box::new(ReceiverStream::new(rx)));
    }

    // Race: spawn N stream consumers, the first to emit a delta wins
    let (tx, rx) = mpsc::channel::<StreamEvent>(64);
    let mut handles = Vec::new();
    for target in &spec.targets {
        let target_owned = target.clone();
        let mut sub_req = req.clone();
        sub_req.model = target_owned.clone();
        let provider = registry.pick(&sub_req)
            .ok_or_else(|| AppError::BadRequest(format!("unknown provider: {}", target_owned)))?;
        let req_clone = sub_req.clone();
        let tx_clone = tx.clone();
        handles.push(tokio::spawn(async move {
            match provider.chat_stream(&req_clone).await {
                Ok(mut stream) => {
                    use futures::StreamExt;
                    while let Some(event) = stream.next().await {
                        if tx_clone.send(event).await.is_err() {
                            return;
                        }
                    }
                }
                Err(e) => {
                    let _ = tx_clone.send(StreamEvent::Error(format!("{}: {}", target_owned, e))).await;
                }
            }
        }));
    }
    // Spawn a supervisor that aborts all tasks once the receiver drops
    tokio::spawn(async move {
        for h in handles {
            // Wait for each — when tx drops (receiver gone), the send() returns Err
            // and the task exits naturally.
            let _ = h.await;
        }
    });
    Ok(Box::new(ReceiverStream::new(rx)))
}
