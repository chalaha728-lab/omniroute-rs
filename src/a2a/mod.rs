//! A2A (Agent-to-Agent) protocol server — enables multi-agent orchestration.
//!
//! Spec: https://a2a-protocol.org/latest/
//!
//! An A2A server exposes "agents" that other agents can discover and invoke.
//! Each agent wraps an LLM + system prompt + tools, and exposes:
//!   - /v1/a2a/agents                   — list agents
//!   - /v1/a2a/agents/:id               — get agent info
//!   - /v1/a2a/agents/:id/invoke        — invoke agent (non-streaming)
//!   - /v1/a2a/agents/:id/stream        — invoke agent (streaming SSE)
//!
//! Built-in agents:
//!   - `default`        — generic assistant (uses the failover registry)
//!   - `coder`          — code-focused (uses coder models with combo:race)
//!   - `researcher`     — research-focused (uses models with web search)
//!   - `summarizer`     — text summarizer

pub mod agents;
pub mod routes;
