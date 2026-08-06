//! Plugin system — extensible hooks for transforming requests/responses.
//!
//! Rust doesn't make dynamic library loading easy (no stable ABI), so we
//! implement a "plugin" as a trait object registered at startup. External
//! plugins would be compiled as Rust crates that depend on this one and
//! register hooks via the PluginRegistry.
//!
//! Hook points (called in order):
//!   - before_request  — mutate the ChatCompletionRequest before failover
//!   - after_response  — mutate the ChatCompletionResponse before returning
//!   - on_error        — observe errors (can't mutate)
//!   - on_usage        — observe usage events (can't mutate)
//!
//! Built-in plugins:
//!   - LoggingPlugin   — logs every request + response (debug level)
//!   - QuotaPlugin     — per-API-key token quota enforcement (TODO)

use async_trait::async_trait;
use once_cell::sync::Lazy;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::error::AppError;
use crate::models::chat::{ChatCompletionRequest, ChatCompletionResponse};
use crate::models::usage::UsageLog;

#[async_trait]
pub trait Plugin: Send + Sync {
    fn name(&self) -> &'static str;
    fn priority(&self) -> i32 { 100 }

    /// Called before the request hits the provider. May mutate the request
    /// (e.g. add a system prompt, redact PII). May return Err to reject.
    async fn before_request(&self, _req: &mut ChatCompletionRequest) -> Result<(), AppError> {
        Ok(())
    }

    /// Called after a successful response. May mutate (e.g. post-process content).
    async fn after_response(&self, _resp: &mut ChatCompletionResponse) -> Result<(), AppError> {
        Ok(())
    }

    /// Called when a request fails (failover exhausted). Observes only.
    async fn on_error(&self, _req: &ChatCompletionRequest, _err: &AppError) {}

    /// Called after usage is recorded. Observes only.
    async fn on_usage(&self, _log: &UsageLog) {}
}

pub mod quota;

pub struct PluginRegistry {
    plugins: Vec<Arc<dyn Plugin>>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self { plugins: Vec::new() }
    }

    pub fn register(&mut self, plugin: Arc<dyn Plugin>) {
        self.plugins.push(plugin);
        // Sort by priority (lower = first)
        self.plugins.sort_by_key(|p| p.priority());
    }

    pub async fn before_request(&self, req: &mut ChatCompletionRequest) -> Result<(), AppError> {
        for p in &self.plugins {
            p.before_request(req).await?;
        }
        Ok(())
    }

    pub async fn after_response(&self, resp: &mut ChatCompletionResponse) -> Result<(), AppError> {
        for p in &self.plugins {
            p.after_response(resp).await?;
        }
        Ok(())
    }

    pub async fn on_error(&self, req: &ChatCompletionRequest, err: &AppError) {
        for p in &self.plugins {
            p.on_error(req, err).await;
        }
    }

    pub async fn on_usage(&self, log: &UsageLog) {
        for p in &self.plugins {
            p.on_usage(log).await;
        }
    }

    pub fn list(&self) -> Vec<&'static str> {
        self.plugins.iter().map(|p| p.name()).collect()
    }
}

pub static PLUGINS: Lazy<RwLock<PluginRegistry>> = Lazy::new(|| {
    let mut reg = PluginRegistry::new();
    reg.register(Arc::new(LoggingPlugin));
    RwLock::new(reg)
});

/// Built-in: logs every request + response at debug level.
pub struct LoggingPlugin;

#[async_trait]
impl Plugin for LoggingPlugin {
    fn name(&self) -> &'static str { "logging" }
    fn priority(&self) -> i32 { 0 } // runs first

    async fn before_request(&self, req: &mut ChatCompletionRequest) -> Result<(), AppError> {
        tracing::debug!(
            "[plugin:logging] before_request model={} messages={}",
            req.model, req.messages.len()
        );
        Ok(())
    }

    async fn after_response(&self, resp: &mut ChatCompletionResponse) -> Result<(), AppError> {
        tracing::debug!(
            "[plugin:logging] after_response model={} choices={} usage={:?}",
            resp.model, resp.choices.len(), resp.usage
        );
        Ok(())
    }

    async fn on_error(&self, req: &ChatCompletionRequest, err: &AppError) {
        tracing::warn!(
            "[plugin:logging] error model={} err={}",
            req.model, err
        );
    }

    async fn on_usage(&self, log: &UsageLog) {
        tracing::debug!(
            "[plugin:logging] usage provider={} model={} tokens={}",
            log.provider_id, log.model, log.total_tokens
        );
    }
}
