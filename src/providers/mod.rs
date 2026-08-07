//! Provider trait + registry + failover logic.
//!
//! Each upstream provider implements `Provider`. The registry holds all
//! configured providers; the failover layer walks them in priority order
//! (per `Config::failover_order`) until one succeeds.

pub mod ai21;
pub mod anthropic;
pub mod anyscale;
pub mod azure;
pub mod baseten;
pub mod bedrock;
pub mod cerebras;
pub mod cohere;
pub mod combo;
pub mod deepinfra;
pub mod deepseek;
pub mod fireworks;
pub mod friendli;
pub mod gemini;
pub mod groq;
pub mod huggingface;
pub mod hyperbolic;
pub mod jina;
pub mod lepton;
pub mod mistral;
pub mod nebius;
pub mod novita;
pub mod octoai;
pub mod ollama;
pub mod openai;
pub mod openrouter;
pub mod ovhcloud;
pub mod perplexity;
pub mod predibase;
pub mod premai;
pub mod replicate;
pub mod runpod;
pub mod sambanova;
pub mod scaleway;
pub mod siliconflow;
pub mod spawning;
pub mod together;
pub mod vertex;
pub mod voyage;
pub mod watsonx;
pub mod xai;

use async_trait::async_trait;
use futures::Stream;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::config::Config;
use crate::error::{AppError, AppResult};
use crate::models::chat::{ChatCompletionRequest, ChatCompletionResponse, StreamEvent};
pub use crate::models::provider::ProviderId;

/// A single upstream LLM provider.
#[async_trait]
pub trait Provider: Send + Sync {
    /// Provider id (e.g. "openai").
    fn id(&self) -> ProviderId;

    /// Whether this provider has API credentials configured and is usable.
    fn is_configured(&self) -> bool;

    /// Non-streaming chat completion.
    async fn chat(&self, req: &ChatCompletionRequest) -> AppResult<ChatCompletionResponse>;

    /// Streaming chat completion. Returns a stream of normalized `StreamEvent`s.
    async fn chat_stream(
        &self,
        req: &ChatCompletionRequest,
    ) -> AppResult<Box<dyn Stream<Item = StreamEvent> + Send + Unpin>>;

    /// List models exposed by this provider (used by /v1/models).
    async fn list_models(&self) -> AppResult<Vec<ModelInfo>>;
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ModelInfo {
    pub id: String,           // "<provider>:<model>" e.g. "openai:gpt-4o"
    pub object: &'static str, // "model"
    pub created: i64,
    pub owned_by: String,
}

// ─── Registry ───────────────────────────────────────────────────────────────

pub struct Registry {
    providers: HashMap<ProviderId, Arc<dyn Provider>>,
    order: Vec<ProviderId>,
}

impl Registry {
    /// Build the registry from config. Only includes providers that have
    /// API keys configured.
    pub fn build(config: &Config) -> Self {
        let mut providers: HashMap<ProviderId, Arc<dyn Provider>> = HashMap::new();

        let candidates: Vec<Arc<dyn Provider>> = vec![
            Arc::new(openai::OpenAI::new(config.provider_keys.openai.clone())),
            Arc::new(anthropic::Anthropic::new(config.provider_keys.anthropic.clone())),
            Arc::new(gemini::Gemini::new(config.provider_keys.gemini.clone())),
            Arc::new(deepseek::new(config.provider_keys.deepseek.clone())),
            Arc::new(openrouter::new(config.provider_keys.openrouter.clone())),
            Arc::new(groq::new(config.provider_keys.groq.clone())),
            Arc::new(mistral::new(config.provider_keys.mistral.clone())),
            Arc::new(xai::new(config.provider_keys.xai.clone())),
            Arc::new(together::new(config.provider_keys.together.clone())),
            Arc::new(fireworks::new(config.provider_keys.fireworks.clone())),
            Arc::new(cohere::Cohere::new(config.provider_keys.cohere.clone())),
            Arc::new(replicate::new(config.provider_keys.replicate.clone())),
            Arc::new(huggingface::new(config.provider_keys.huggingface.clone())),
            Arc::new(ai21::new(config.provider_keys.ai21.clone())),
            Arc::new(perplexity::new(config.provider_keys.perplexity.clone())),
            Arc::new(azure::Azure::new(config.provider_keys.azure.clone())),
            Arc::new(ollama::new(config.provider_keys.ollama.clone())),
            Arc::new(cerebras::new(config.provider_keys.cerebras.clone())),
            Arc::new(novita::new(config.provider_keys.novita.clone())),
            Arc::new(sambanova::new(config.provider_keys.sambanova.clone())),
            Arc::new(siliconflow::new(config.provider_keys.siliconflow.clone())),
            Arc::new(lepton::new(config.provider_keys.lepton.clone())),
            Arc::new(deepinfra::new(config.provider_keys.deepinfra.clone())),
            Arc::new(nebius::new(config.provider_keys.nebius.clone())),
            Arc::new(hyperbolic::new(config.provider_keys.hyperbolic.clone())),
            Arc::new(bedrock::new(config.provider_keys.bedrock.clone())),
            Arc::new(vertex::new(config.provider_keys.vertex.clone())),
            Arc::new(voyage::new(config.provider_keys.voyage.clone())),
            Arc::new(jina::new(config.provider_keys.jina.clone())),
            Arc::new(watsonx::new(config.provider_keys.watsonx.clone())),
            Arc::new(anyscale::new(config.provider_keys.anyscale.clone())),
            Arc::new(friendli::new(config.provider_keys.friendli.clone())),
            Arc::new(baseten::new(config.provider_keys.baseten.clone())),
            Arc::new(octoai::new(config.provider_keys.octoai.clone())),
            Arc::new(predibase::new(config.provider_keys.predibase.clone())),
            Arc::new(runpod::new(config.provider_keys.runpod.clone())),
            Arc::new(premai::new(config.provider_keys.premai.clone())),
            Arc::new(spawning::new(config.provider_keys.spawning.clone())),
            Arc::new(scaleway::new(config.provider_keys.scaleway.clone())),
            Arc::new(ovhcloud::new(config.provider_keys.ovhcloud.clone())),
        ];

        for p in candidates {
            if p.is_configured() {
                providers.insert(p.id(), p);
            }
        }

        // Resolve the failover order — drop any providers not actually configured.
        let order: Vec<ProviderId> = config
            .failover_order
            .iter()
            .filter_map(|s| ProviderId::from_str(s))
            .filter(|p| providers.contains_key(p))
            .collect();

        tracing::info!(
            "[registry] {} providers configured: [{}]",
            providers.len(),
            order.iter().map(|p| p.as_str()).collect::<Vec<_>>().join(", ")
        );

        Self { providers, order }
    }

    pub fn get(&self, id: ProviderId) -> Option<Arc<dyn Provider>> {
        self.providers.get(&id).cloned()
    }

    pub fn all(&self) -> Vec<Arc<dyn Provider>> {
        self.order.iter().filter_map(|id| self.providers.get(id).cloned()).collect()
    }

    pub fn order(&self) -> &[ProviderId] {
        &self.order
    }

    /// Pick the provider for a request. If the request's model is `provider:model`,
    /// use that provider. Otherwise return None (caller should walk all providers).
    pub fn pick(&self, req: &ChatCompletionRequest) -> Option<Arc<dyn Provider>> {
        let (provider_hint, _model_name) = req.split_model();
        if let Some(pid) = provider_hint.and_then(ProviderId::from_str) {
            return self.get(pid);
        }
        None
    }
}

// ─── Failover ───────────────────────────────────────────────────────────────

/// Shared registry wrapper — Arc<RwLock<Registry>> so we can hot-reload providers
/// later (e.g. when a user adds a key via the dashboard).
pub type SharedRegistry = Arc<RwLock<Registry>>;

/// Try the request on each configured provider in order until one succeeds.
/// If `req.model` is `provider:model`, only that provider is tried.
pub async fn chat_with_failover(
    registry: &Registry,
    req: &ChatCompletionRequest,
    max_retries: u32,
) -> AppResult<ChatCompletionResponse> {
    let candidates = if let Some(p) = registry.pick(req) {
        vec![p]
    } else {
        registry.all()
    };

    if candidates.is_empty() {
        return Err(AppError::AllProvidersFailed);
    }

    let mut last_err: Option<AppError> = None;
    for provider in &candidates {
        for attempt in 0..=max_retries {
            match provider.chat(req).await {
                Ok(resp) => return Ok(resp),
                Err(e) => {
                    tracing::warn!(
                        "[failover] {} attempt {} failed: {}",
                        provider.id(),
                        attempt + 1,
                        e
                    );
                    last_err = Some(e);
                    // Only retry on transient errors (5xx, timeouts). 4xx errors
                    // (bad request, auth) should fail over immediately.
                    // For simplicity we retry on every error here.
                }
            }
        }
    }
    Err(last_err.unwrap_or(AppError::AllProvidersFailed))
}

/// Streaming failover: try each provider; on error before the first chunk,
/// move to the next provider. Once the first chunk is emitted, the stream is
/// committed to that provider — mid-stream errors are surfaced as `StreamEvent::Error`.
pub async fn chat_stream_with_failover(
    registry: &Registry,
    req: &ChatCompletionRequest,
    _max_retries: u32,
) -> AppResult<Box<dyn Stream<Item = StreamEvent> + Send + Unpin>> {
    let candidates = if let Some(p) = registry.pick(req) {
        vec![p]
    } else {
        registry.all()
    };

    if candidates.is_empty() {
        return Err(AppError::AllProvidersFailed);
    }

    // For streaming, we commit to the first provider that returns a stream
    // successfully. Mid-stream failures surface as StreamEvent::Error.
    let mut last_err: Option<AppError> = None;
    for provider in &candidates {
        match provider.chat_stream(req).await {
            Ok(stream) => return Ok(stream),
            Err(e) => {
                tracing::warn!(
                    "[failover] {} stream init failed: {}",
                    provider.id(),
                    e
                );
                last_err = Some(e);
            }
        }
    }
    Err(last_err.unwrap_or(AppError::AllProvidersFailed))
}

/// Helper: list all models from all configured providers.
pub async fn list_all_models(registry: &Registry) -> Vec<ModelInfo> {
    let mut out = Vec::new();
    for provider in registry.all() {
        match provider.list_models().await {
            Ok(models) => out.extend(models),
            Err(e) => tracing::warn!("[models] {} list_models failed: {}", provider.id(), e),
        }
    }
    out
}
