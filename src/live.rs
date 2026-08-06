//! Live WebSocket dashboard — real-time usage + provider status broadcasts.
//!
//! Clients connect to ws://localhost:20128/ws/dashboard and receive JSON events:
//!   { "type": "usage",         "data": { ... } }
//!   { "type": "provider_status","data": { "provider_id": "...", "status": "..." } }
//!   { "type": "ping" }   (keepalive every 30s)
//!
//! Server-side: a broadcast::Sender is shared globally. Whenever a request
//! completes or a provider status changes, fire a message. Subscribers receive
//! a copy.

use axum::extract::ws::{Message as WsMessage, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use futures::{SinkExt, StreamExt};
use once_cell::sync::Lazy;
use serde::Serialize;
use serde_json::json;
use tokio::sync::broadcast;

/// Shared broadcast channel — 1024 message buffer (subscribers slower than
/// this will miss messages, which is fine for a dashboard).
static TX: Lazy<broadcast::Sender<DashEvent>> = Lazy::new(|| {
    let (tx, _rx) = broadcast::channel(1024);
    tx
});

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum DashEvent {
    Usage {
        provider_id: String,
        model: String,
        prompt_tokens: u32,
        completion_tokens: u32,
        duration_ms: u64,
        status_code: u16,
    },
    ProviderStatus {
        provider_id: String,
        status: String, // "ok" | "failed" | "rate_limited"
        message: Option<String>,
    },
    Ping,
}

/// Broadcast an event to all connected dashboard subscribers.
pub fn broadcast(event: DashEvent) {
    // send() returns Err when there are no subscribers — that's fine.
    let _ = TX.send(event);
}

/// Convenience helpers.
pub fn broadcast_usage(
    provider_id: &str,
    model: &str,
    prompt_tokens: u32,
    completion_tokens: u32,
    duration_ms: u64,
    status_code: u16,
) {
    broadcast(DashEvent::Usage {
        provider_id: provider_id.into(),
        model: model.into(),
        prompt_tokens, completion_tokens, duration_ms, status_code,
    });
}

pub fn broadcast_provider_status(provider_id: &str, status: &str, message: Option<String>) {
    broadcast(DashEvent::ProviderStatus {
        provider_id: provider_id.into(),
        status: status.into(),
        message,
    });
}

/// WebSocket upgrade handler: GET /ws/dashboard
pub async fn ws_dashboard(
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(handle_socket)
}

async fn handle_socket(mut socket: WebSocket) {
    let mut rx = TX.subscribe();

    // Send an initial hello so the client knows the connection is live.
    let hello = json!({
        "type": "hello",
        "data": { "server": "omniroute-rust", "version": env!("CARGO_PKG_VERSION") }
    }).to_string();
    let _ = socket.send(WsMessage::Text(hello)).await;

    // Spawn a heartbeat every 30s.
    let mut heartbeat = tokio::time::interval(std::time::Duration::from_secs(30));

    loop {
        tokio::select! {
            // Receive broadcast events and forward to client.
            Ok(event) = rx.recv() => {
                let json = serde_json::to_string(&event).unwrap_or_else(|_| "{}".into());
                if socket.send(WsMessage::Text(json)).await.is_err() {
                    break;
                }
            }
            // Heartbeat.
            _ = heartbeat.tick() => {
                let ping = json!({ "type": "ping" }).to_string();
                if socket.send(WsMessage::Text(ping)).await.is_err() {
                    break;
                }
            }
            // Client messages (we don't expect any, but keep the connection alive).
            Some(msg) = socket.recv() => {
                match msg {
                    Ok(WsMessage::Ping(_)) | Ok(WsMessage::Pong(_)) | Ok(WsMessage::Close(_)) => continue,
                    Ok(_) => continue,
                    Err(_) => break,
                }
            }
        }
    }
}
