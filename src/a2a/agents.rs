//! Built-in A2A agents — wrap an LLM + system prompt + tools.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use once_cell::sync::Lazy;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub id: String,
    pub name: String,
    pub description: String,
    pub system_prompt: String,
    /// Default model — uses failover if no provider prefix.
    pub model: String,
    pub temperature: f32,
    pub max_tokens: u32,
    /// Skills/capabilities the agent advertises.
    pub skills: Vec<String>,
    /// Optional tools the agent can call (MCP-style).
    pub tools: Vec<String>,
}

pub static AGENTS: Lazy<RwLock<HashMap<String, Agent>>> = Lazy::new(|| {
    let mut m = HashMap::new();
    m.insert("default".into(), Agent {
        id: "default".into(),
        name: "Default Assistant".into(),
        description: "General-purpose AI assistant with multi-provider failover.".into(),
        system_prompt: "You are a helpful AI assistant. Answer the user's question clearly and concisely.".into(),
        model: "openai:gpt-4o-mini".into(),
        temperature: 0.7,
        max_tokens: 4096,
        skills: vec!["chat".into(), "reasoning".into(), "writing".into()],
        tools: vec![],
    });
    m.insert("coder".into(), Agent {
        id: "coder".into(),
        name: "Code Assistant".into(),
        description: "Code-focused assistant that races across multiple coder-optimized models.".into(),
        system_prompt: "You are a senior software engineer. Write clean, idiomatic, well-tested code. Explain your reasoning briefly before the code, then provide the implementation in a fenced block.".into(),
        model: "combo:race:openai:gpt-4o,anthropic:claude-3-5-sonnet-20241022,deepseek:deepseek-chat".into(),
        temperature: 0.2,
        max_tokens: 8192,
        skills: vec!["code-generation".into(), "code-review".into(), "debugging".into()],
        tools: vec![],
    });
    m.insert("researcher".into(), Agent {
        id: "researcher".into(),
        name: "Research Assistant".into(),
        description: "Research-focused assistant using models with built-in web search.".into(),
        system_prompt: "You are a research assistant. Synthesize information from multiple sources, cite your sources, and present a balanced view. If you're unsure, say so.".into(),
        model: "perplexity:sonar-pro".into(),
        temperature: 0.3,
        max_tokens: 8192,
        skills: vec!["web-search".into(), "synthesis".into(), "citation".into()],
        tools: vec![],
    });
    m.insert("summarizer".into(), Agent {
        id: "summarizer".into(),
        name: "Text Summarizer".into(),
        description: "Condenses long text into bullet points + a one-paragraph TL;DR.".into(),
        system_prompt: "You are a text summarizer. Read the input and produce:\n1. A one-paragraph TL;DR (2-3 sentences)\n2. 3-5 bullet points covering the key points\nKeep it concise. Don't add information not in the source.".into(),
        model: "anthropic:claude-3-5-haiku-20241022".into(),
        temperature: 0.0,
        max_tokens: 1024,
        skills: vec!["summarization".into(), "extraction".into()],
        tools: vec![],
    });
    RwLock::new(m)
});

pub async fn list() -> Vec<Agent> {
    AGENTS.read().await.values().cloned().collect()
}

pub async fn get(id: &str) -> Option<Agent> {
    AGENTS.read().await.get(id).cloned()
}

pub async fn register(agent: Agent) {
    AGENTS.write().await.insert(agent.id.clone(), agent);
}

pub async fn unregister(id: &str) -> bool {
    AGENTS.write().await.remove(id).is_some()
}
