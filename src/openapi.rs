//! OpenAPI 3.0 spec endpoint at /api/openapi.json.
//!
//! Generates a static OpenAPI document describing every route in OmniRoute-Rust.
//! Clients can use this to auto-generate SDKs in any language (openapi-generator).

use axum::Json;
use serde_json::{json, Value};
use crate::error::AppResult;

pub async fn openapi_spec() -> AppResult<Json<Value>> {
    Ok(Json(json!({
        "openapi": "3.0.3",
        "info": {
            "title": "OmniRoute-Rust API",
            "version": env!("CARGO_PKG_VERSION"),
            "description": "Fast, light, OpenAI-compatible AI gateway — pure Rust rewrite. Single binary, no runtime deps.",
            "license": { "name": "MIT", "url": "https://opensource.org/license/mit" },
            "homepage": "https://omniroute.online"
        },
        "servers": [
            { "url": "http://localhost:20128", "description": "Local server" }
        ],
        "components": {
            "securitySchemes": {
                "BearerAuth": {
                    "type": "http",
                    "scheme": "bearer",
                    "description": "Use a JWT (from /api/auth/login) or an API key (sk-or-...)"
                }
            }
        },
        "paths": {
            "/v1/chat/completions": {
                "post": {
                    "summary": "Chat completion",
                    "description": "OpenAI-compatible chat completion. Supports streaming (SSE), combos, multi-provider failover, compression, guardrails.",
                    "security": [{ "BearerAuth": [] }],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/ChatCompletionRequest" }
                            }
                        }
                    },
                    "responses": {
                        "200": { "description": "Successful response (or SSE stream if stream=true)" }
                    }
                }
            },
            "/v1/models": {
                "get": {
                    "summary": "List models",
                    "security": [{ "BearerAuth": [] }],
                    "responses": { "200": { "description": "List of available models" } }
                }
            },
            "/v1/embeddings": {
                "post": {
                    "summary": "Create embeddings",
                    "security": [{ "BearerAuth": [] }],
                    "responses": { "200": { "description": "Embedding vectors" } }
                }
            },
            "/v1/audio/speech": {
                "post": {
                    "summary": "Text-to-speech",
                    "description": "Returns audio bytes (mp3/opus/aac/flac/wav)",
                    "security": [{ "BearerAuth": [] }],
                    "responses": { "200": { "description": "Audio bytes" } }
                }
            },
            "/v1/images/generations": {
                "post": {
                    "summary": "Generate image (DALL-E)",
                    "security": [{ "BearerAuth": [] }],
                    "responses": { "200": { "description": "Image URLs or base64" } }
                }
            },
            "/v1/mcp/sse": {
                "get": {
                    "summary": "MCP SSE endpoint",
                    "description": "Server-Sent Events stream for MCP clients"
                }
            },
            "/v1/mcp/messages": {
                "post": {
                    "summary": "MCP JSON-RPC messages",
                    "description": "Accepts JSON-RPC 2.0 requests for tool listing and invocation"
                }
            },
            "/v1/a2a/agents": {
                "get": { "summary": "List A2A agents" },
                "post": { "summary": "Register a new A2A agent", "security": [{ "BearerAuth": [] }] }
            },
            "/v1/a2a/agents/{id}": {
                "get": { "summary": "Get agent details" },
                "delete": { "summary": "Delete an agent", "security": [{ "BearerAuth": [] }] }
            },
            "/v1/a2a/agents/{id}/invoke": {
                "post": {
                    "summary": "Invoke an agent",
                    "security": [{ "BearerAuth": [] }]
                }
            },
            "/api/auth/login": {
                "post": {
                    "summary": "Login → JWT",
                    "requestBody": { "content": { "application/json": { "schema": { "type": "object", "properties": { "username": {"type":"string"}, "password": {"type":"string"} } } } } }
                }
            },
            "/api/auth/verify": {
                "get": { "summary": "Verify JWT", "security": [{ "BearerAuth": [] }] }
            },
            "/api/auth/password": {
                "post": { "summary": "Change password", "security": [{ "BearerAuth": [] }] }
            },
            "/api/dashboard/usage": {
                "get": { "summary": "Usage stats by provider", "security": [{ "BearerAuth": [] }] }
            },
            "/api/dashboard/providers": {
                "get": { "summary": "List configured providers", "security": [{ "BearerAuth": [] }] }
            },
            "/api/dashboard/providers/{id}": {
                "put": { "summary": "Update provider config", "security": [{ "BearerAuth": [] }] }
            },
            "/api/dashboard/api-keys": {
                "get": { "summary": "List API keys", "security": [{ "BearerAuth": [] }] },
                "post": { "summary": "Create API key", "security": [{ "BearerAuth": [] }] }
            },
            "/api/dashboard/api-keys/{id}": {
                "delete": { "summary": "Revoke API key", "security": [{ "BearerAuth": [] }] }
            },
            "/api/dashboard/api-keys/{id}/quota": {
                "get": { "summary": "Get API key quota", "security": [{ "BearerAuth": [] }] },
                "put": { "summary": "Set API key quota", "security": [{ "BearerAuth": [] }] }
            },
            "/api/dashboard/health": {
                "get": { "summary": "List provider health", "security": [{ "BearerAuth": [] }] }
            },
            "/api/dashboard/health/{id}": {
                "get": { "summary": "Get provider health", "security": [{ "BearerAuth": [] }] }
            },
            "/api/dashboard/pricing": {
                "get": { "summary": "List all model prices", "security": [{ "BearerAuth": [] }] }
            },
            "/api/dashboard/pricing/{provider}/{model}": {
                "put": { "summary": "Override a model price", "security": [{ "BearerAuth": [] }] }
            },
            "/api/dashboard/orgs": {
                "get": { "summary": "List orgs", "security": [{ "BearerAuth": [] }] },
                "post": { "summary": "Create org", "security": [{ "BearerAuth": [] }] }
            },
            "/api/dashboard/orgs/{id}": {
                "get": { "summary": "Get org", "security": [{ "BearerAuth": [] }] },
                "put": { "summary": "Update org", "security": [{ "BearerAuth": [] }] },
                "delete": { "summary": "Delete org", "security": [{ "BearerAuth": [] }] }
            },
            "/api/dashboard/orgs/{id}/members": {
                "post": { "summary": "Add member", "security": [{ "BearerAuth": [] }] }
            },
            "/api/dashboard/orgs/{id}/members/{user_id}": {
                "delete": { "summary": "Remove member", "security": [{ "BearerAuth": [] }] }
            },
            "/api/dashboard/orgs/{id}/usage": {
                "get": { "summary": "Get org usage", "security": [{ "BearerAuth": [] }] }
            },
            "/api/dashboard/systemd/install": {
                "post": { "summary": "Install systemd service (Linux)", "security": [{ "BearerAuth": [] }] }
            },
            "/api/dashboard/systemd/uninstall": {
                "post": { "summary": "Uninstall systemd service", "security": [{ "BearerAuth": [] }] }
            },
            "/api/dashboard/systemd/status": {
                "get": { "summary": "Check systemd status", "security": [{ "BearerAuth": [] }] }
            },
            "/api/dashboard/tunnel/start": {
                "post": { "summary": "Start Cloudflare tunnel", "security": [{ "BearerAuth": [] }] }
            },
            "/api/dashboard/tunnel/stop": {
                "post": { "summary": "Stop Cloudflare tunnel", "security": [{ "BearerAuth": [] }] }
            },
            "/api/dashboard/tunnel/status": {
                "get": { "summary": "Check tunnel status", "security": [{ "BearerAuth": [] }] }
            },
            "/api/dashboard/plugins": {
                "get": { "summary": "List installed plugins", "security": [{ "BearerAuth": [] }] }
            },
            "/api/dashboard/whoami": {
                "get": { "summary": "Current auth context", "security": [{ "BearerAuth": [] }] }
            },
            "/api/monitoring/health": {
                "get": { "summary": "Liveness + DB ping" }
            },
            "/metrics": {
                "get": { "summary": "Prometheus metrics" }
            },
            "/api/openapi.json": {
                "get": { "summary": "OpenAPI 3.0 spec (this document)" }
            },
            "/ws/dashboard": {
                "get": { "summary": "Live WebSocket dashboard (usage + provider status events)" }
            }
        },
        "components": {
            "schemas": {
                "ChatCompletionRequest": {
                    "type": "object",
                    "required": ["model", "messages"],
                    "properties": {
                        "model": { "type": "string", "description": "Model id, e.g. 'openai:gpt-4o' or 'combo:race:openai:gpt-4o,anthropic:claude-3-5-sonnet'" },
                        "messages": { "type": "array", "items": { "type": "object" } },
                        "temperature": { "type": "number", "default": 1.0 },
                        "top_p": { "type": "number", "default": 1.0 },
                        "max_tokens": { "type": "integer" },
                        "stream": { "type": "boolean", "default": false },
                        "stop": { "type": "string" },
                        "seed": { "type": "integer" },
                        "tools": { "type": "array" },
                        "tool_choice": {}
                    }
                }
            }
        }
    })))
}
